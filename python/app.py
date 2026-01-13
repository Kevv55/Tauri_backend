# python/app.py
from flask import Flask, jsonify, request
import random
import time
import threading

app = Flask(__name__)

# Store application state
class AppState:
    def __init__(self):
        self.counter = 0
        self.running = False
        self.lock = threading.Lock()
    
    def increment_counter(self):
        with self.lock:
            self.counter += 1
            return self.counter
    
    def reset_counter(self):
        with self.lock:
            self.counter = 0

state = AppState()


def get_lucky_number():
    """Generate a random lucky number for the user"""
    lucky_number = random.randint(1, 100)
    message = f"Here is your lucky number: {lucky_number}"
    return message


def echo_user_input(user_input):
    """Return the user's input back to them"""
    message = f"You said: {user_input}"
    return message


@app.route('/status', methods=['GET'])
def get_status():
    """Return current status update with lucky number"""
    count = state.increment_counter()
    
    return jsonify({
        "type": "status",
        "message": get_lucky_number(),
        "count": count,
        "timestamp": time.time()
    })


@app.route('/input', methods=['POST'])
def handle_input():
    """Handle user input and echo it back"""
    data = request.get_json()
    
    if not data or 'input' not in data:
        return jsonify({"error": "No input provided"}), 400
    
    user_input = data['input']
    count = state.increment_counter()
    
    return jsonify({
        "type": "user_input",
        "message": echo_user_input(user_input),
        "input": user_input,
        "count": count,
        "timestamp": time.time()
    })


@app.route('/stop', methods=['POST'])
def stop_server():
    """Gracefully stop the server"""
    state.running = False
    return jsonify({"status": "stopping"})


@app.route('/health', methods=['GET'])
def health_check():
    """Health check endpoint"""
    return jsonify({"status": "ok"})


if __name__ == '__main__':
    print("Starting Flask server on localhost:5000...")
    state.running = True
    app.run(host='127.0.0.1', port=5000, debug=False, threaded=True)