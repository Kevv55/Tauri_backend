@echo off
REM python/build_binary.bat
REM Script to build Python Flask app as standalone executable for Windows

setlocal enabledelayedexpansion

echo Building Python Flask server as standalone binary...

REM Activate venv if it exists
if exist ".venv" (
    call .venv\Scripts\activate.bat
)

REM Install PyInstaller if not already installed
pip install pyinstaller

REM Change to python directory
cd python

REM Build for Windows
echo Building for Windows...
pyinstaller pyinstaller.spec

REM Move to binaries directory
if not exist "..\src-tauri\binaries" mkdir ..\src-tauri\binaries

REM Determine architecture
for /f "tokens=*" %%i in ('wmic os get osarchitecture ^| findstr /R /C:[0-9]') do set ARCH=%%i
if "%ARCH%"=="64-bit" (
    set TARGET_NAME=ai-engine-x86_64-pc-windows-msvc.exe
) else (
    set TARGET_NAME=ai-engine-i686-pc-windows-msvc.exe
)

copy dist\ai-engine.exe ..\src-tauri\binaries\%TARGET_NAME%
echo Binary created: binaries\%TARGET_NAME%

echo Build complete!
