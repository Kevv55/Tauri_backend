#!/bin/bash
# python/build_binary.sh
# Script to build Python Starlette/Hypercorn app as standalone executable

set -e

echo "Building Python AI Engine as standalone binary..."

# Activate venv if it exists
if [ -d ".venv" ]; then
    source .venv/bin/activate
fi

# Clean PyInstaller cache to ensure fresh rebuild
echo "Cleaning PyInstaller cache..."
rm -rf python/build python/dist build/ai-engine

# Install PyInstaller if not already installed
pip install pyinstaller

# Change to python directory
cd python

# Build for current platform
echo "Building for $(uname -s) $(uname -m)..."
pyinstaller pyinstaller.spec

# Move to binaries directory
mkdir -p ../src-tauri/binaries

# Get architecture and normalize to Rust convention
# uname -m returns: arm64 (Apple Silicon), x86_64 (Intel), etc.
# Rust uses: aarch64 (ARM 64-bit), x86_64, etc.
ARCH=$(uname -m)
if [ "$ARCH" == "arm64" ]; then
    ARCH="aarch64"  # Normalize Apple Silicon to Rust convention
fi

if [ "$(uname)" == "Darwin" ]; then
    # macOS
    TARGET_NAME="ai-engine-${ARCH}-apple-darwin"
    cp dist/ai-engine "../src-tauri/binaries/${TARGET_NAME}"
    chmod +x "../src-tauri/binaries/${TARGET_NAME}"
    echo "Binary created: binaries/${TARGET_NAME}"
elif [ "$(uname)" == "Linux" ]; then
    # Linux
    TARGET_NAME="ai-engine-${ARCH}-unknown-linux-gnu"
    cp dist/ai-engine "../src-tauri/binaries/${TARGET_NAME}"
    chmod +x "../src-tauri/binaries/${TARGET_NAME}"
    echo "Binary created: binaries/${TARGET_NAME}"
fi

echo "Build complete!"
