#!/bin/bash
# python/build_binary.sh
# Script to build Python Flask app as standalone executable

set -e

echo "Building Python Flask server as standalone binary..."

# Activate venv if it exists
if [ -d ".venv" ]; then
    source .venv/bin/activate
fi

# Install PyInstaller if not already installed
pip install pyinstaller

# Change to python directory
cd python

# Build for current platform
echo "Building for $(uname -s) $(uname -m)..."
pyinstaller pyinstaller.spec

# Move to binaries directory
mkdir -p ../src-tauri/binaries
if [ "$(uname)" == "Darwin" ]; then
    # macOS: Get architecture
    ARCH=$(uname -m)
    TARGET_NAME="ai-engine-${ARCH}-apple-darwin"
    cp dist/ai-engine "../src-tauri/binaries/${TARGET_NAME}"
    chmod +x "../src-tauri/binaries/${TARGET_NAME}"
    echo "Binary created: binaries/${TARGET_NAME}"
elif [ "$(uname)" == "Linux" ]; then
    # Linux
    ARCH=$(uname -m)
    TARGET_NAME="ai-engine-${ARCH}-unknown-linux-gnu"
    cp dist/ai-engine "../src-tauri/binaries/${TARGET_NAME}"
    chmod +x "../src-tauri/binaries/${TARGET_NAME}"
    echo "Binary created: binaries/${TARGET_NAME}"
fi

echo "Build complete!"
