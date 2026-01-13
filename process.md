## High-Level Architecture Overview

The application uses a **three-tier architecture** with React frontend, Rust backend (Tauri), and Python Flask sidecar:

1. **Frontend (React/TypeScript)**: User interface with start/stop buttons and input form
2. **Backend (Rust/Tauri)**: Process manager that spawns Python, handles HTTP communication, and emits events
3. **Sidecar (Python/Flask)**: HTTP server running on localhost:5000 that processes requests

Communication happens via:
- **Tauri Commands**: Frontend → Rust (invoke)
- **HTTP**: Rust ↔ Flask (RESTful API)
- **Events**: Rust → Frontend (emit "python_status", "python_input")

## Architecture Flow

```
User starts app
    ↓
Clicks "Start Python Script"
    ↓
Frontend: invoke("start_python_script")
    ↓
Rust: Spawns child process: python ../python/app.py
    ↓
Python: Flask starts listening on localhost:5000
    ↓
Rust: Health check - polls GET /health (20 retries, 500ms intervals)
    ↓
Flask: Returns {"status": "ok"}
    ↓
Rust: Starts polling loop (1 second intervals)
    ↓
Polling Thread: Continuously sends GET /status
    ↓
Flask: Returns latest lucky number and timestamp
    ↓
Rust: Emits event "python_status" → Frontend updates display
```

## START Process (`start_python_script`)

**Triggered by**: User clicks "Start Python Script" button

**Flow**:
1. Frontend calls `invoke("start_python_script")`
2. Rust checks if Flask already running - if yes, returns early
3. Rust spawns subprocess: `python ../venv/bin/python3 ../python/app.py`
4. Rust waits for Flask to be ready:
   - Makes GET requests to `http://127.0.0.1:5000/health`
   - Retries up to 20 times (10 seconds total)
   - If fails, returns error to frontend
5. Rust marks `is_running = true`
6. Rust spawns async polling task that:
   - Runs every 1 second for the lifetime of the app
   - Checks idle timeout (explained below)
   - Polls `/status` endpoint
   - Emits "python_status" event to frontend

## STOP Process (`stop_python_script`)

**Can be triggered by**: 
- User clicks "Stop Python Script" button (manual stop)
- Automatic idle timeout (automatic stop)

**Manual Stop Flow**:
1. Frontend calls `invoke("stop_python_script")`
2. Rust sends POST request to `http://127.0.0.1:5000/stop`
3. Python gracefully shuts down
4. Rust waits 500ms for graceful shutdown
5. Rust forcefully kills the process if still running
6. Rust marks `is_running = false`
7. Polling task exits

## IDLE TIMEOUT (Automatic Stop)

**Mechanism**: Flask automatically stops after 5 minutes of inactivity

**How it works**:
1. Every command that interacts with Flask updates `last_activity` timestamp
   - `start_python_script()` - sets initial timestamp
   - `send_input_to_python()` - user sends input (resets timer)
   - `on_app_interaction()` - any app interaction (optional)

2. Polling thread checks every 1 second:
   ```
   elapsed_time = current_time - last_activity_timestamp
   
   if elapsed_time > 5 minutes (300 seconds):
       - POST /stop to Flask (graceful shutdown)
       - Mark is_running = false
       - Exit polling loop
   ```

3. If user sends input while idle:
   - Timestamp is updated
   - Flask continues running
   - Timer resets

**Example Timeline**:
```
14:00 - User clicks Start → last_activity = 14:00
14:02 - User sends input → last_activity = 14:02
14:03 - No activity → polling checks: 14:03 - 14:02 = 1 min < 5 min ✓ (continues)
14:07 - No activity → polling checks: 14:07 - 14:02 = 5 min ✓ (equals timeout)
14:07 - Flask auto-stops → frontend shows "stopped"
```

**Benefits**:
- Prevents resource waste from forgotten instances
- Automatic cleanup without user intervention
- User can restart by clicking "Start" again



## Input Flow (`send_input_to_python`)

**Triggered by**: User types text and clicks "Send Input" button

**Flow**:
1. Frontend calls `invoke("send_input_to_python", { input })`
2. Rust updates `last_activity` timestamp (resets idle timer)
3. Rust sends POST to `http://127.0.0.1:5000/input` with JSON: `{"input": "user text"}`
4. Flask processes input and returns: `{"input": "user text", "message": "echo response", ...}`
5. Rust emits "python_input" event with the response
6. Frontend receives event and displays in InputCard
7. Frontend clears the input field

## Key Features

- **Health Checks**: Flask readiness verified before polling starts
- **Activity Tracking**: All interactions reset the 5-minute idle timer
- **Graceful Shutdown**: Flask receives stop signal before forceful termination
- **Event-Driven**: Frontend always updated via events, not polling
- **Error Handling**: Clear error messages if Flask fails to start

## Checklist
- ✅ Architecture implemented with health checks
- ✅ Idle timeout with activity tracking
- ✅ Graceful shutdown mechanism
- ✅ Event-based communication (python_status, python_input)
- ⏳ Production binary generation (needs PyInstaller compilation)

## To-do's
- Create CI/CD pipeline to automate binary generation
- Cross-platform testing (macOS, Windows, Linux)
- Binary code signing and notarization
- Auto-update mechanism


Flask architecture breakdown
┌─────────────────────────────────────────────────────────────────┐
│                    TAURI APPLICATION                            │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │              FRONTEND (React/TypeScript)                  │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │  App.tsx                                            │  │  │
│  │  │  - Start/Stop buttons                               │  │  │
│  │  │  - Input form                                       │  │  │
│  │  │  - Display Status/Input cards                       │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  └────────────────┬──────────────────────────────────────────┘  │
│                   │ invoke() HTTP/Tauri API                      │
│  ┌────────────────▼──────────────────────────────────────────┐  │
│  │           RUST BACKEND (Tauri + lib.rs)                  │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │ Commands:                                           │  │  │
│  │  │ - start_python_script()                             │  │  │
│  │  │ - stop_python_script()                              │  │  │
│  │  │ - send_input_to_python()                            │  │  │
│  │  │                                                      │  │  │
│  │  │ Actions:                                            │  │  │
│  │  │ 1. Spawn Python process (localhost:5000)            │  │  │
│  │  │ 2. Make HTTP requests to Flask                      │  │  │
│  │  │ 3. Parse responses                                  │  │  │
│  │  │ 4. Emit events back to frontend                     │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  └────────────┬──────────────────────────────┬────────────────┘  │
│               │ HTTP Requests                │ Events (emit)     │
│               │ (localhost:5000)             │ (python_status,   │
│               │                              │  python_input)    │
└───────────────┼──────────────────────────────┼───────────────────┘
                │                              │
    ┌───────────▼──────────────┐     ┌─────────▲───────────┐
    │  PYTHON BACKEND (Flask)  │     │   FRONTEND (listens)│
    │  ┌────────────────────┐  │     │                     │
    │  │ Flask Server       │  │     │  Event Listeners:   │
    │  │ :5000              │  │     │  - python_status    │
    │  │                    │  │     │  - python_input     │
    │  │ Endpoints:         │  │     └─────────────────────┘
    │  │ GET  /status       │  │
    │  │ POST /input        │  │
    │  │ POST /stop         │  │
    │  └────────────────────┘  │
    │  ┌────────────────────┐  │
    │  │ Background Tasks   │  │
    │  │ - Generate lucky   │  │
    │  │   numbers          │  │
    │  │ - Echo user input  │  │
    │  │ - Emit updates     │  │
    │  └────────────────────┘  │
    └────────────────────────────┘


Process lifecyle:
App Start
  ↓
User clicks "Start"
  ↓
Rust spawns: python python/app.py
  ↓
Python starts Flask on :5000
  ↓
Rust polls GET :5000/status (1 sec interval)
  ↓
Python returns status → Rust emits event → Frontend updates
  ↓
User sends input
  ↓
Rust POST :5000/input → Python processes → returns response
  ↓
Rust emits event → Frontend updates
  ↓
User clicks "Stop"
  ↓
Rust POST :5000/stop
  ↓
Python gracefully shuts down
  ↓
Rust kills process, stops polling