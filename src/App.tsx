import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

interface PythonOutput {
  type?: string;
  message?: string;
  input?: string;
  count?: number;
  timestamp?: number;
}

function StatusCard({ data }: { data: PythonOutput }) {
  const formatTime = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleTimeString();
  };

  return (
    <div className="output-card">
      <div className="card-row">
        <span className="label">Message:</span>
        <span className="value">{data.message || "-"}</span>
      </div>
      <div className="card-row">
        <span className="label">Count:</span>
        <span className="value">{data.count || 0}</span>
      </div>
      <div className="card-row">
        <span className="label">Time:</span>
        <span className="value">{data.timestamp ? formatTime(data.timestamp) : "-"}</span>
      </div>
    </div>
  );
}

function InputCard({ data }: { data: PythonOutput }) {
  const formatTime = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleTimeString();
  };

  return (
    <div className="output-card input-response-card">
      <div className="card-row">
        <span className="label">Your Input:</span>
        <span className="value">{data.input || "-"}</span>
      </div>
      <div className="card-row">
        <span className="label">Echo:</span>
        <span className="value">{data.message || "-"}</span>
      </div>
      <div className="card-row">
        <span className="label">Time:</span>
        <span className="value">{data.timestamp ? formatTime(data.timestamp) : "-"}</span>
      </div>
    </div>
  );
}

function App() {
  const [statusOutput, setStatusOutput] = useState<PythonOutput | null>(null);
  const [inputOutput, setInputOutput] = useState<PythonOutput | null>(null);
  const [input, setInput] = useState<string>("");
  const [isRunning, setIsRunning] = useState<boolean>(false);
  const unlistenStatusRef = useRef<(() => void) | null>(null);
  const unlistenInputRef = useRef<(() => void) | null>(null);

  async function startPython() {
    try {
      setStatusOutput(null);
      setInputOutput(null);
      
      // Set up listener for status updates BEFORE starting the script
      const unlistenStatus = await listen("python_status", (event) => {
        try {
          const parsed = JSON.parse(event.payload as string);
          setStatusOutput(parsed);
        } catch {
          setStatusOutput({ message: event.payload as string });
        }
      });
      unlistenStatusRef.current = unlistenStatus;

      // Set up listener for user input responses
      const unlistenInput = await listen("python_input", (event) => {
        try {
          const parsed = JSON.parse(event.payload as string);
          setInputOutput(parsed);
        } catch {
          setInputOutput({ message: event.payload as string });
        }
      });
      unlistenInputRef.current = unlistenInput;

      // Start the Python script
      await invoke("start_python_script");
      setIsRunning(true);
    } catch (error) {
      console.error(error);
      setStatusOutput({ message: "Error starting Python: " + String(error) });
      setIsRunning(false);
      if (unlistenStatusRef.current) {
        unlistenStatusRef.current();
        unlistenStatusRef.current = null;
      }
      if (unlistenInputRef.current) {
        unlistenInputRef.current();
        unlistenInputRef.current = null;
      }
    }
  }

  async function stopPython() {
    try {
      await invoke("stop_python_script");
      setIsRunning(false);
      setStatusOutput({ message: "Python script stopped" });
      
      // Clean up listeners
      if (unlistenStatusRef.current) {
        unlistenStatusRef.current();
        unlistenStatusRef.current = null;
      }
      if (unlistenInputRef.current) {
        unlistenInputRef.current();
        unlistenInputRef.current = null;
      }
    } catch (error) {
      console.error(error);
      setStatusOutput({ message: "Error stopping Python: " + String(error) });
    }
  }

  async function sendInput(e: React.FormEvent) {
    e.preventDefault();
    if (!input.trim()) return;

    try {
      await invoke("send_input_to_python", { input: input });
      setInput("");
    } catch (error) {
      console.error(error);
      setStatusOutput({ message: "Error sending input: " + String(error) });
    }
  }

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (unlistenStatusRef.current) {
        unlistenStatusRef.current();
      }
      if (unlistenInputRef.current) {
        unlistenInputRef.current();
      }
    };
  }, []);

  return (
    <div className="container">
      <h1>Tauri + Python Sidecar</h1>

      <div className="card">
        {!isRunning ? (
          <button onClick={startPython}>Start Python Script</button>
        ) : (
          <button onClick={stopPython}>Stop Python Script</button>
        )}
      </div>

      <div className="output-box">
        <h3>Status Updates:</h3>
        {statusOutput ? (
          <StatusCard data={statusOutput} />
        ) : (
          <div className="output-card">
            <p style={{ color: "white" }}>Waiting for status updates...</p>
          </div>
        )}
      </div>

      <div className="output-box">
        <h3>Your Input Echo:</h3>
        {inputOutput ? (
          <InputCard data={inputOutput} />
        ) : (
          <div className="output-card">
            <p style={{ color: "white" }}>Your inputs will appear here...</p>
          </div>
        )}
      </div>

      <form onSubmit={sendInput} className="input-form">
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Enter input for Python script..."
          disabled={!isRunning}
        />
        <button type="submit" disabled={!isRunning}>
          Send Input
        </button>
      </form>
    </div>
  );
}

export default App;