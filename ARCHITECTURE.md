# Tauri + Flask Backend Architecture

## Overview

This project uses a **Rust (Tauri) frontend** with a **Python (Flask) backend** communicating via HTTP.

## Key Features Implemented

### ✅ Development Mode (Current)
- Python runs from source code (`.venv/bin/python3` or `.venv\Scripts\python.exe`)
- Flask server on `localhost:5000`
- Hot reload compatible
- Easy debugging

### ✅ Lifecycle Management
- **Auto-idle detection**: Flask stops after 5 minutes of inactivity
- **Health checks**: Waits for Flask to be ready before polling
- **Graceful shutdown**: Notifies Flask to stop before killing process
- **Activity tracking**: Updated on user interactions

### ✅ Production Features (Ready to implement)
- Convert Flask to standalone binary with PyInstaller
- Automatic binary naming for different platforms
- CI/CD ready

---

## Development Setup

### 1. Install Python Dependencies
```bash
pip install -r python/requirements.txt
```

### 2. Create Virtual Environment (Recommended)
```bash
python3 -m venv .venv

# Activate
source .venv/bin/activate  # macOS/Linux
# or
.venv\Scripts\activate.bat  # Windows
```

### 3. Install Rust Dependencies
```bash
cd src-tauri
cargo build
```

### 4. Run Development
```bash
# From project root
bun tauri dev
# or
npm tauri dev
```

---

## Building Production Binary

### Step 1: Build Flask Binary (One-time setup)

**macOS/Linux:**
```bash
cd python
bash build_binary.sh
```

**Windows:**
```bash
cd python
build_binary.bat
```

This creates a standalone executable at `src-tauri/binaries/ai-engine-<platform>`

### Step 2: Update Rust to use binary
Edit `src-tauri/src/lib.rs`:
```rust
// Change from:
let python_exe = get_python_executable();

// To:
let python_exe = "binaries/ai-engine";  // Tauri will bundle this
```

### Step 3: Build Tauri App
```bash
bun tauri build
```

---

## Architecture Diagram

```
Frontend (React)
    ↓ invoke()
Rust (Tauri) - manages lifecycle
    ↓ HTTP polls
Backend (Flask) - :5000
    ↓
Python logic (get_lucky_number, echo_input)
```

---

## Configuration Constants (in lib.rs)

```rust
const IDLE_TIMEOUT_SECS: u64 = 300;              // Stop after 5 min idle
const HEALTH_CHECK_RETRIES: u32 = 20;            // 20 retries for Flask startup
const HEALTH_CHECK_INTERVAL_MS: u64 = 500;       // 500ms between retries
```

---

## Flask Endpoints

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/health` | Health check |
| GET | `/status` | Get lucky number |
| POST | `/input` | Send user input |
| POST | `/stop` | Graceful shutdown |

---

## Important Rust Concepts Used

### 1. `Mutex<T>` - Thread-Safe State
```rust
state: Arc<Mutex<bool>>  // Shared, thread-safe boolean
```

### 2. `Arc<T>` - Atomic Reference Counting
```rust
last_activity: Arc<Mutex<Instant>>  // Shared ownership across async tasks
```

### 3. Async Tasks with `spawn()`
```rust
tauri::async_runtime::spawn(async move { ... })  // Long-running polling task
```

### 4. Platform-Specific Conditionals
```rust
#[cfg(target_os = "windows")]  // Windows-only code
#[cfg(not(target_os = "windows"))]  // Unix-like only
```

---

## Troubleshooting

### Flask fails to start
- Check if port 5000 is already in use: `lsof -i :5000`
- Verify Python executable: `which .venv/bin/python3`
- Check Flask logs in console

### Health check timeout
- Flask might be slow to start
- Increase `HEALTH_CHECK_RETRIES` in lib.rs
- Check system resources

### App keeps stopping after 5 minutes
- This is intentional! Idle timeout is working
- Interact with app to reset timer
- Change `IDLE_TIMEOUT_SECS` if needed

---

## Next Steps: CI/CD Pipeline

Will implement:
1. **GitHub Actions** for automated builds
2. **Platform-specific builds** (macOS, Windows, Linux)
3. **Artifact upload** to releases
4. **Auto-update** mechanism

---

## File Structure
```
├── src-tauri/src/lib.rs          # Rust backend + lifecycle management
├── python/
│   ├── app.py                    # Flask server
│   ├── requirements.txt          # Python dependencies + PyInstaller
│   ├── pyinstaller.spec          # Build configuration
│   ├── build_binary.sh           # macOS/Linux build script
│   └── build_binary.bat          # Windows build script
└── src/App.tsx                   # Frontend (unchanged)
```
