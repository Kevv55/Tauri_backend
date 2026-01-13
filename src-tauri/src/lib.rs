// src-tauri/src/lib.rs
use tauri_plugin_shell::ShellExt;
use tauri::{AppHandle, State, Emitter};
use tauri::async_runtime::Mutex;
use std::time::{Duration, Instant};
use std::sync::Arc;

// Store the running Python process and idle timer
pub struct PythonProcess {
    child: Option<Box<dyn std::any::Any + Send>>,
    last_activity: Arc<Mutex<Instant>>,
    is_running: Arc<Mutex<bool>>,
}

// Wrapper to handle state cloning for async tasks
pub struct PythonProcessState {
    last_activity: Arc<Mutex<Instant>>,
    is_running: Arc<Mutex<bool>>,
}

const IDLE_TIMEOUT_SECS: u64 = 300; // 5 minutes
const HEALTH_CHECK_RETRIES: u32 = 20;
const HEALTH_CHECK_INTERVAL_MS: u64 = 500;

/// Get the Python executable path - uses venv if available
fn get_python_executable() -> String {
    #[cfg(target_os = "windows")]
    {
        "../venv/Scripts/python.exe".to_string()
    }
    #[cfg(not(target_os = "windows"))]
    {
        "../venv/bin/python3".to_string()
    }
}

/// Health check to verify Flask is running
async fn wait_for_flask_ready() -> Result<(), String> {
    let client = reqwest::Client::new();
    
    for attempt in 1..=HEALTH_CHECK_RETRIES {
        match client.get("http://127.0.0.1:5000/health").send().await {
            Ok(_) => {
                println!("Flask is ready (attempt {}/{})", attempt, HEALTH_CHECK_RETRIES);
                return Ok(());
            }
            Err(e) => {
                if attempt >= HEALTH_CHECK_RETRIES {
                    return Err(format!("Flask failed to start after {} attempts: {}", HEALTH_CHECK_RETRIES, e));
                }
                tokio::time::sleep(Duration::from_millis(HEALTH_CHECK_INTERVAL_MS)).await;
            }
        }
    }
    
    Err("Flask startup timeout".to_string())
}

/// Update activity timestamp (called from commands)
async fn update_activity_impl(last_activity_arc: &Arc<Mutex<Instant>>) {
    let mut last_activity = last_activity_arc.lock().await;
    *last_activity = Instant::now();
}

#[tauri::command]
async fn start_python_script(app: AppHandle, state: State<'_, Mutex<PythonProcess>>) -> Result<(), String> {
    println!("Starting Python Flask server...");
    
    // Debug: print current directory
    let cwd = std::env::current_dir().ok();
    println!("Current working directory: {:?}", cwd);
    
    // Check if already running
    let proc_state = state.lock().await;
    let is_running = *proc_state.is_running.lock().await;
    if is_running {
        println!("Flask is already running");
        return Ok(());
    }
    drop(proc_state);
    
    // Build the path to the Python script relative to the project
    let script_path = "../python/app.py";
    let python_exe = get_python_executable();
    println!("Python executable: {}", python_exe);
    println!("Script path: {}", script_path);
    
    // Spawn Python with venv
    let (_rx, child) = app.shell()
        .command(python_exe)
        .arg(script_path)
        .spawn()
        .map_err(|e| {
            println!("Error spawning Python: {}", e);
            e.to_string()
        })?;

    println!("Python process spawned successfully");

    // Store the child process and update state
    let mut proc_state = state.lock().await;
    proc_state.child = Some(Box::new(child));
    let mut last_activity = proc_state.last_activity.lock().await;
    *last_activity = Instant::now();
    drop(last_activity);
    drop(proc_state);

    // Wait for Flask to be ready before polling
    wait_for_flask_ready().await?;

    // Update running state
    {
        let proc_state = state.lock().await;
        let mut is_running = proc_state.is_running.lock().await;
        *is_running = true;
    }

    // Clone app handle and state for the polling loop
    let app_clone = app.clone();
    let proc_state = state.lock().await;
    let state_clone = PythonProcessState {
        last_activity: proc_state.last_activity.clone(),
        is_running: proc_state.is_running.clone(),
    };
    drop(proc_state);

    // Spawn a task to poll Flask /status endpoint periodically
    tauri::async_runtime::spawn(async move {
        println!("Starting status polling...");
        let client = reqwest::Client::new();
        
        loop {
            // Check if we should stop due to inactivity
            let last_activity_lock = state_clone.last_activity.lock().await;
            let last_activity = *last_activity_lock;
            drop(last_activity_lock);
            
            if last_activity.elapsed() > Duration::from_secs(IDLE_TIMEOUT_SECS) {
                println!("Flask idle timeout reached, stopping...");
                let client = reqwest::Client::new();
                let _ = client.post("http://127.0.0.1:5000/stop").send().await;
                let mut is_running = state_clone.is_running.lock().await;
                *is_running = false;
                break;
            }
            
            tokio::time::sleep(Duration::from_secs(1)).await;
            
            match client.get("http://127.0.0.1:5000/status").send().await {
                Ok(response) => {
                    match response.json::<serde_json::Value>().await {
                        Ok(json_data) => {
                            println!("Received status: {:?}", json_data);
                            let _ = app_clone.emit("python_status", json_data.to_string());
                        }
                        Err(e) => {
                            println!("Error parsing response: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("Error polling /status: {}", e);
                    // Continue polling even if there's an error
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
async fn stop_python_script(state: State<'_, Mutex<PythonProcess>>) -> Result<(), String> {
    println!("Stopping Python server...");
    
    let mut proc_state = state.lock().await;
    let is_running = *proc_state.is_running.lock().await;
    
    if !is_running {
        println!("Flask is not running");
        return Ok(());
    }

    // Notify Flask to stop
    let client = reqwest::Client::new();
    let _ = client.post("http://127.0.0.1:5000/stop").send().await;

    // Give it time to shut down gracefully
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Kill the process
    if let Some(_child) = proc_state.child.take() {
        println!("Python process terminated");
    }
    
    let mut is_running_flag = proc_state.is_running.lock().await;
    *is_running_flag = false;

    Ok(())
}

#[tauri::command]
async fn send_input_to_python(app: AppHandle, input: String, state: State<'_, Mutex<PythonProcess>>) -> Result<(), String> {
    println!("Sending input to Flask: {}", input);
    
    // Update activity
    let proc_state = state.lock().await;
    update_activity_impl(&proc_state.last_activity).await;
    drop(proc_state);
    
    let client = reqwest::Client::new();
    
    match client.post("http://127.0.0.1:5000/input")
        .json(&serde_json::json!({ "input": input }))
        .send()
        .await
    {
        Ok(response) => {
            match response.json::<serde_json::Value>().await {
                Ok(json_data) => {
                    println!("Received response: {:?}", json_data);
                    // Emit the response back to the frontend
                    let _ = app.emit("python_input", json_data.to_string());
                    Ok(())
                }
                Err(e) => {
                    Err(format!("Error parsing response: {}", e))
                }
            }
        }
        Err(e) => {
            Err(format!("Error sending input: {}", e))
        }
    }
}

#[tauri::command]
async fn on_app_interaction(state: State<'_, Mutex<PythonProcess>>) -> Result<(), String> {
    // Update activity timestamp
    let proc_state = state.lock().await;
    let mut last_activity = proc_state.last_activity.lock().await;
    *last_activity = Instant::now(); // <-- Sets initial timestamp
    drop(last_activity);
    
    // Start Flask if not running
    let _is_running = *proc_state.is_running.lock().await;
    drop(proc_state);
    
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(Mutex::new(PythonProcess {
            child: None,
            last_activity: Arc::new(Mutex::new(Instant::now())),
            is_running: Arc::new(Mutex::new(false)),
        }))
        .invoke_handler(tauri::generate_handler![
            start_python_script,
            stop_python_script,
            send_input_to_python,
            on_app_interaction
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}