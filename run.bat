@echo off
setlocal
cd /d "%~dp0"

if exist "%~dp0to-avif.exe" (
    start "" "%~dp0to-avif.exe" %*
) else if exist "%~dp0src-tauri\target\release\to-avif.exe" (
    start "" "%~dp0src-tauri\target\release\to-avif.exe" %*
) else (
    echo [ERROR] to-avif.exe not found.
    echo Please run setup.bat or build.bat first.
    pause
)
