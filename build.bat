@echo off
setlocal
cd /d "%~dp0"

echo Building To-avif (Rust Release Mode)...
echo.

cargo build --release --manifest-path src-tauri/Cargo.toml

if %errorlevel% equ 0 (
    echo.
    echo ========================================================
    echo   Build completed successfully!
    echo   Deploying to-avif.exe to root directory...
    copy /y "%~dp0src-tauri\target\release\to-avif.exe" "%~dp0to-avif.exe" >nul
    echo   Executable ready: to-avif.exe
    echo ========================================================
) else (
    echo.
    echo [ERROR] Build failed.
)

echo.
pause
