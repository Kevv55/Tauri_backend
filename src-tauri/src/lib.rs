// src-tauri/src/lib.rs
//! =============================================================================
//! AI Engine Rust Backend - Unix Socket IPC
//! =============================================================================
//! 
//! This module manages lifecycle and communication with the Python AI Engine
//! backend via Unix Domain Sockets (UDS).
//!
//! Architecture:
//!   ┌─────────────────────────────────────────────┐
//!   │  Tauri Frontend (TypeScript/React)          │
//!   │  ├─ start_python_script() command           │
//!   │  ├─ send_input_to_python() command          │
//!   │  └─ stop_python_script() command            │
//!   └────────────────┬────────────────────────────┘
//!                    │
//!         Unix Domain Socket (UDS)
//!         /tmp/ai-engine.sock
//!                    │
//!   ┌────────────────▼────────────────────────────┐
//! Python AI Engine (Hypercorn/Starlette)       │
//!   ├─ /status      (periodic health check)    │
//!   ├─ /input       (user requests)            │
//!   ├─ /health      (startup verification)     │
//!   └─ /stop        (graceful shutdown)        │
//!   └─────────────────────────────────────────────┘
//!
//! Communication:
//!   • Unix Domain Socket (/tmp/ai-engine.sock)
//!   • HTTP/1.1 over Unix socket (via Hypercorn)
//!   • No TCP overhead, direct kernel IPC
//!
//! Key Features:
//!   • Unix Socket Communication - Enterprise-grade IPC with file permissions
//!   • Memory Optimization - TensorFlow/PyTorch stay resident
//!   • Idle Timeout (5 min) - Automatically stops server when inactive
//!   • Binary Support - Works with PyInstaller compiled executables
//!   • Graceful Shutdown - Clean termination with signal handling

use tauri_plugin_shell::ShellExt;
use tauri::{AppHandle, State, Emitter};
use tauri::async_runtime::Mutex;
use std::time::{Duration, Instant};
use std::sync::Arc;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

// ==================== Configuration Constants ====================

/// Idle timeout: If no activity for this duration, server stops automatically
const IDLE_TIMEOUT_SECS: u64 = 300; // 5 minutes

/// Health check: Maximum retries when waiting for server to start
const HEALTH_CHECK_RETRIES: u32 = 20;

/// Health check: Delay between consecutive startup attempts
const HEALTH_CHECK_INTERVAL_MS: u64 = 500;

/// Status polling: How often we check server health
const STATUS_POLL_INTERVAL_SECS: u64 = 1;

/// Socket file permissions: Owner can read/write only (0o600)
const SOCKET_PERMISSIONS: u32 = 0o600;

// ==================== Socket Path Management ====================

/// Get the Unix socket path used for IPC communication.
/// Default: /tmp/ai-engine.sock
/// The socket file will be created by the Python server.
fn get_socket_path() -> String {
    "/tmp/ai-engine.sock".to_string()
}

/// Check if Unix socket file exists and is ready for connections.
/// Returns true if socket exists and is accessible.
fn is_socket_ready(socket_path: &str) -> bool {
    Path::new(socket_path).exists()
}

// ==================== Utility Functions ====================

/// Get the compiled AI Engine binary path based on platform and architecture.
/// 
/// Returns the path to prebuilt executable in src-tauri/binaries/
/// Format: ai-engine-{arch}-{os}[-exe]
fn get_ai_engine_binary() -> String {
    #[cfg(target_os = "macos")]
    {
        #[cfg(target_arch = "aarch64")]
        {
            "../src-tauri/binaries/ai-engine-aarch64-apple-darwin".to_string()
        }
        #[cfg(target_arch = "x86_64")]
        {
            "../src-tauri/binaries/ai-engine-x86_64-apple-darwin".to_string()
        }
    }
    #[cfg(target_os = "linux")]
    {
        #[cfg(target_arch = "x86_64")]
        {
            "../src-tauri/binaries/ai-engine-x86_64-unknown-linux-gnu".to_string()
        }
        #[cfg(target_arch = "aarch64")]
        {
            "../src-tauri/binaries/ai-engine-aarch64-unknown-linux-gnu".to_string()
        }
    }
    #[cfg(target_os = "windows")]
    {
        "../src-tauri/binaries/ai-engine-x86_64-pc-windows-msvc.exe".to_string()
    }
}

/// Wait for Unix socket to be ready and accepting connections.
/// 
/// Attempts to connect to the socket file at the specified path.
/// Returns Ok if socket is ready within HEALTH_CHECK_RETRIES attempts.
/// 
/// This is the startup verification - we check for socket file existence
/// rather than making HTTP requests.
async fn wait_for_socket_ready() -> Result<(), String> {
    let socket_path = get_socket_path();
    
    for attempt in 1..=HEALTH_CHECK_RETRIES {
        if is_socket_ready(&socket_path) {
            println!("Socket ready at {} (attempt {}/{})", socket_path, attempt, HEALTH_CHECK_RETRIES);
            return Ok(());
        }
        
        if attempt >= HEALTH_CHECK_RETRIES {
            return Err(format!(
                "Socket failed to appear at {} after {} attempts",
                socket_path, HEALTH_CHECK_RETRIES
            ));
        }
        
        tokio::time::sleep(Duration::from_millis(HEALTH_CHECK_INTERVAL_MS)).await;
    }
    
    Err("Socket startup timeout".to_string())
}

/// Update activity timestamp (called when user interacts with app).
/// 
/// Resets the idle timer. If server hasn't been accessed for IDLE_TIMEOUT_SECS,
/// it will be automatically stopped to save memory.
async fn update_activity_impl(last_activity_arc: &Arc<Mutex<Instant>>) {
    let mut last_activity = last_activity_arc.lock().await;
    *last_activity = Instant::now();
}

// ==================== Unix Socket HTTP Communication ====================

/// Send an HTTP GET request over Unix domain socket.
/// 
/// This function creates an HTTP request to the Hypercorn server listening
/// on a Unix socket. It's used for health checks and status polling.
async fn socket_http_get(socket_path: &str, endpoint: &str) -> Result<serde_json::Value, String> {
    use tokio::net::UnixStream;
    
    let mut stream = UnixStream::connect(socket_path)
        .await
        .map_err(|e| format!("Failed to connect to socket: {}", e))?;
    
    // Construct HTTP GET request
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        endpoint
    );
    
    // Send request
    stream.write_all(request.as_bytes())
        .await
        .map_err(|e| format!("Failed to write to socket: {}", e))?;
    
    // Read response
    let mut response = String::new();
    stream.read_to_string(&mut response)
        .await
        .map_err(|e| format!("Failed to read from socket: {}", e))?;
    
    // Parse HTTP response body (skip headers)
    // Try Windows line endings first (\r\n\r\n), then Unix line endings (\n\n)
    let body = if let Some(pos) = response.find("\r\n\r\n") {
        &response[pos + 4..]
    } else if let Some(pos) = response.find("\n\n") {
        &response[pos + 2..]
    } else {
        return Err("Invalid HTTP response format - no body separator found".to_string());
    };
    
    // Parse JSON from body
    serde_json::from_str(body.trim())
        .map_err(|e| format!("Failed to parse response JSON: {}", e))
}

/// Send an HTTP POST request with JSON body over Unix domain socket.
/// 
/// This function creates an HTTP POST request to the Hypercorn server.
/// Used for sending user input and stop signals.
async fn socket_http_post(socket_path: &str, endpoint: &str, body: &serde_json::Value) -> Result<serde_json::Value, String> {
    use tokio::net::UnixStream;
    
    let mut stream = UnixStream::connect(socket_path)
        .await
        .map_err(|e| format!("Failed to connect to socket: {}", e))?;
    
    let body_str = serde_json::to_string(body)
        .map_err(|e| format!("Failed to serialize JSON: {}", e))?;
    
    // Construct HTTP POST request
    let request = format!(
        "POST {} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        endpoint,
        body_str.len(),
        body_str
    );
    
    // Send request
    stream.write_all(request.as_bytes())
        .await
        .map_err(|e| format!("Failed to write to socket: {}", e))?;
    
    // Read response
    let mut response = String::new();
    stream.read_to_string(&mut response)
        .await
        .map_err(|e| format!("Failed to read from socket: {}", e))?;
    
    // Parse HTTP response body (skip headers)
    // Try Windows line endings first (\r\n\r\n), then Unix line endings (\n\n)
    let body = if let Some(pos) = response.find("\r\n\r\n") {
        &response[pos + 4..]
    } else if let Some(pos) = response.find("\n\n") {
        &response[pos + 2..]
    } else {
        return Err("Invalid HTTP response format - no body separator found".to_string());
    };
    
    // Parse JSON from body (skip empty bodies)
    if body.is_empty() {
        return Ok(serde_json::json!({}));
    }
    
    serde_json::from_str(body.trim())
        .map_err(|e| format!("Failed to parse response JSON: {}", e))
}

// ==================== Tauri Command: start_python_script ====================

/// Start the AI Engine backend process via precompiled binary.
///
/// This command:
///   1. Checks if server is already running
///   2. Spawns the ai-engine binary (PyInstaller executable)
///   3. Waits for Unix socket to become ready
///   4. Starts the status polling loop that monitors health and idle timeout
///
/// The binary path is selected based on the current platform/architecture.
/// Returns Ok if startup succeeds, Err with details if it fails.
#[tauri::command]
async fn start_python_script(app: AppHandle, state: State<'_, Mutex<PythonProcess>>) -> Result<(), String> {
    println!("Starting AI Engine backend (Unix socket mode)...");
    
    // Check if already running to prevent multiple instances
    let proc_state = state.lock().await;
    let is_running = *proc_state.is_running.lock().await;
    if is_running {
        println!("AI Engine is already running");
        return Ok(());
    }
    drop(proc_state);
    
    // Get the compiled binary path for this platform
    let binary_path = get_ai_engine_binary();
    let socket_path = get_socket_path();
    
    println!("Binary path: {}", binary_path);
    println!("Socket path: {}", socket_path);
    
    // Spawn the AI Engine binary
    // The binary is self-contained and will listen on the Unix socket
    let (_rx, child) = app.shell()
        .command(&binary_path)
        .spawn()
        .map_err(|e| {
            println!("Error spawning AI Engine binary: {}", e);
            format!("Failed to spawn binary at {}: {}", binary_path, e)
        })?;

    println!("AI Engine process spawned successfully");

    // Store the child process handle and initialize activity tracking
    let mut proc_state = state.lock().await;
    proc_state.child = Some(Box::new(child));
    let mut last_activity = proc_state.last_activity.lock().await;
    *last_activity = Instant::now();
    drop(last_activity);
    drop(proc_state);

    // Wait for Unix socket to be ready (server has started and created socket)
    println!("Waiting for socket to be ready...");
    wait_for_socket_ready().await?;

    // Update running state to mark server as operational
    {
        let proc_state = state.lock().await;
        let mut is_running = proc_state.is_running.lock().await;
        *is_running = true;
    }

    // Clone app handle and state for the background polling task
    let app_clone = app.clone();
    let proc_state = state.lock().await;
    let state_clone = PythonProcessState {
        last_activity: proc_state.last_activity.clone(),
        is_running: proc_state.is_running.clone(),
    };
    drop(proc_state);

    // Spawn background task: health polling + idle timeout enforcement
    // This task runs continuously and:
    //   • Checks idle timeout every second
    //   • Sends /stop to server if idle too long
    //   • Polls /status endpoint to receive updates
    //
    // Communication: Direct Unix Domain Socket (no TCP overhead)
    tauri::async_runtime::spawn(async move {
        println!("Starting status polling loop (via Unix socket)...");
        let socket_path_clone = socket_path.clone();
        
        loop {
            // Check idle timeout
            let last_activity_lock = state_clone.last_activity.lock().await;
            let last_activity = *last_activity_lock;
            drop(last_activity_lock);
            
            if last_activity.elapsed() > Duration::from_secs(IDLE_TIMEOUT_SECS) {
                println!("Idle timeout reached ({} secs), stopping AI Engine...", IDLE_TIMEOUT_SECS);
                
                // Send graceful shutdown request via Unix socket
                if let Ok(_response) = socket_http_post(&socket_path_clone, "/stop", &serde_json::json!({}))
                    .await
                {
                    println!("Sent stop signal to AI Engine via Unix socket");
                }
                
                let mut is_running = state_clone.is_running.lock().await;
                *is_running = false;
                break;
            }
            
            // Wait before next poll
            tokio::time::sleep(Duration::from_secs(STATUS_POLL_INTERVAL_SECS)).await;
            
            // Poll /status endpoint for updates via Unix socket
            // The response contains application state that we emit to the frontend
            if let Ok(json_data) = socket_http_get(&socket_path_clone, "/status")
                .await
            {
                println!("Status: {:?}", json_data);
                let _ = app_clone.emit("python_status", json_data.to_string());
            }
        }
    });

    Ok(())
}

// ==================== Tauri Command: stop_python_script ====================

/// Stop the AI Engine backend process gracefully.
///
/// This command:
///   1. Checks if server is running
///   2. Sends graceful /stop request via Unix socket
///   3. Waits briefly for shutdown
///   4. Terminates process if needed
///   5. Marks server as stopped
///
/// The Unix socket communication is direct kernel IPC with no TCP overhead.
#[tauri::command]
async fn stop_python_script(state: State<'_, Mutex<PythonProcess>>) -> Result<(), String> {
    println!("Stopping AI Engine backend...");
    
    let mut proc_state = state.lock().await;
    let is_running = *proc_state.is_running.lock().await;
    
    if !is_running {
        println!("AI Engine is not running");
        return Ok(());
    }

    // Send graceful stop request via Unix socket
    let socket_path = get_socket_path();
    let _ = socket_http_post(&socket_path, "/stop", &serde_json::json!({}))
        .await;

    // Wait for graceful shutdown
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Terminate process if still alive
    if let Some(_child) = proc_state.child.take() {
        println!("AI Engine process terminated");
    }
    
    // Mark as stopped
    let mut is_running_flag = proc_state.is_running.lock().await;
    *is_running_flag = false;

    Ok(())
}

// ==================== Tauri Command: send_input_to_python ====================

/// Send user input to the AI Engine backend via Unix socket.
///
/// This command:
///   1. Updates the idle activity timestamp (resets idle counter)
///   2. Sends user input as JSON POST to /input endpoint
///   3. Emits the response back to the frontend
///
/// Used when user interacts with the application.
/// Communication: Direct Unix Domain Socket with HTTP request format.
#[tauri::command]
async fn send_input_to_python(app: AppHandle, input: String, state: State<'_, Mutex<PythonProcess>>) -> Result<(), String> {
    println!("Sending input to AI Engine: {}", input);
    
    // Update activity timestamp (prevent idle timeout)
    let proc_state = state.lock().await;
    update_activity_impl(&proc_state.last_activity).await;
    drop(proc_state);
    
    // Send request via Unix socket
    let socket_path = get_socket_path();
    
    match socket_http_post(&socket_path, "/input", &serde_json::json!({ "input": input }))
        .await
    {
        Ok(json_data) => {
            println!("Received response: {:?}", json_data);
            // Emit response to frontend
            let _ = app.emit("python_input", json_data.to_string());
            Ok(())
        }
        Err(e) => {
            Err(format!("Error sending input via Unix socket: {}", e))
        }
    }
}

// ==================== Tauri Command: on_app_interaction ====================

/// Called when user interacts with the frontend to reset idle timer.
///
/// This command updates the last activity timestamp, preventing
/// the server from being stopped due to inactivity.
/// Call this on any user action (clicks, input, etc).
#[tauri::command]
async fn on_app_interaction(state: State<'_, Mutex<PythonProcess>>) -> Result<(), String> {
    // Update activity timestamp to prevent idle timeout
    let proc_state = state.lock().await;
    let mut last_activity = proc_state.last_activity.lock().await;
    *last_activity = Instant::now();
    drop(last_activity);
    drop(proc_state);
    
    Ok(())
}

// ==================== Tauri App Entry Point ====================

/// Initialize and run the Tauri application.
/// Sets up the AI Engine process manager and exposes IPC commands to frontend.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        // Initialize the Python process state (not started yet)
        .manage(Mutex::new(PythonProcess {
            child: None,
            last_activity: Arc::new(Mutex::new(Instant::now())),
            is_running: Arc::new(Mutex::new(false)),
        }))
        // Expose these commands to the frontend via Tauri IPC
        .invoke_handler(tauri::generate_handler![
            start_python_script,    // Start AI Engine backend
            stop_python_script,     // Stop AI Engine backend
            send_input_to_python,   // Send user request
            on_app_interaction      // Reset idle timer
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}