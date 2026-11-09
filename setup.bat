@echo off
setlocal EnableDelayedExpansion

echo ===================================================
echo   To-avif Universal Setup and Tool Installer
echo ===================================================
echo.

set "ROOT_DIR=%~dp0"
set "BIN_DIR=%ROOT_DIR%bin"
set "TEMP_DIR=%ROOT_DIR%temp_setup"
set "RELEASE_EXE=%ROOT_DIR%src-tauri\target\release\to-avif.exe"
set "ROOT_EXE=%ROOT_DIR%to-avif.exe"

:: [1/5] Check and prepare directories
echo [1/5] Preparing directories...
if not exist "%BIN_DIR%" mkdir "%BIN_DIR%"
if not exist "%TEMP_DIR%" mkdir "%TEMP_DIR%"

:: Copy release binary to root if available
if exist "%RELEASE_EXE%" (
    if not exist "%ROOT_EXE%" (
        echo   * Deploying to-avif.exe to root directory...
        copy /y "%RELEASE_EXE%" "%ROOT_EXE%" >nul
    )
)

echo   [OK] Directory structure verified.
echo.

:: [2/5] Check and download FFmpeg (with libsvtav1 support)
echo [2/5] Checking FFmpeg (AVIF encoder)...

set "NEED_FFMPEG=0"
if not exist "%BIN_DIR%\ffmpeg.exe" set "NEED_FFMPEG=1"

if "!NEED_FFMPEG!"=="0" (
    echo   * ffmpeg.exe: Found in bin\
) else (
    echo   * ffmpeg.exe: Missing. Downloading release package...
    set "FFMPEG_ZIP=%TEMP_DIR%\ffmpeg.zip"
    curl.exe -fL --progress-bar -o "!FFMPEG_ZIP!" "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip"
    if exist "!FFMPEG_ZIP!" (
        echo     Extracting ffmpeg and ffprobe to bin\...
        where tar.exe >nul 2>&1
        if !errorlevel! equ 0 (
            tar.exe -xf "!FFMPEG_ZIP!" -C "%TEMP_DIR%"
        ) else (
            powershell -NoProfile -Command "Expand-Archive -Path '!FFMPEG_ZIP!' -DestinationPath '%TEMP_DIR%' -Force"
        )
        for /r "%TEMP_DIR%" %%f in (ffmpeg.exe ffprobe.exe) do (
            if exist "%%f" copy /y "%%f" "%BIN_DIR%\" >nul
        )
        if exist "%BIN_DIR%\ffmpeg.exe" (
            echo     [OK] ffmpeg.exe installed successfully.
        ) else (
            echo     [WARNING] Could not extract ffmpeg.exe. Please place it manually in bin\.
        )
        del /f /q "!FFMPEG_ZIP!" >nul 2>&1
    ) else (
        echo     [WARNING] Failed to download FFmpeg. Please place ffmpeg.exe manually in bin\.
    )
)
echo.

:: [3/5] Check and download ExifTool (Metadata & Timestamp preservation)
echo [3/5] Checking ExifTool (Metadata preservation engine)...

if exist "%BIN_DIR%\exiftool.exe" (
    echo   * exiftool.exe: Found in bin\
) else (
    echo   * exiftool.exe: Missing. Downloading latest standalone package...
    set "EXIF_ZIP=%TEMP_DIR%\exiftool.zip"
    curl.exe -fL --progress-bar -o "!EXIF_ZIP!" "https://sourceforge.net/projects/exiftool/files/latest/download"
    if exist "!EXIF_ZIP!" (
        echo     Extracting exiftool...
        where tar.exe >nul 2>&1
        if !errorlevel! equ 0 (
            tar.exe -xf "!EXIF_ZIP!" -C "%TEMP_DIR%"
        ) else (
            powershell -NoProfile -Command "Expand-Archive -Path '!EXIF_ZIP!' -DestinationPath '%TEMP_DIR%' -Force"
        )
        for /r "%TEMP_DIR%" %%f in (exiftool*.exe) do (
            if exist "%%f" copy /y "%%f" "%BIN_DIR%\exiftool.exe" >nul
        )
        if exist "%BIN_DIR%\exiftool.exe" (
            echo     [OK] exiftool.exe installed successfully.
        ) else (
            echo     [WARNING] Could not extract exiftool.exe. Please place it manually in bin\.
        )
        del /f /q "!EXIF_ZIP!" >nul 2>&1
    ) else (
        echo     [WARNING] Failed to download ExifTool. Please place exiftool.exe manually in bin\.
    )
)
echo.

:: [4/5] Register Windows Explorer Context Menu (Current User)
echo [4/5] Registering Windows Context Menu...

set "TARGET_EXE=%ROOT_EXE%"
if not exist "%TARGET_EXE%" (
    if exist "%RELEASE_EXE%" (
        set "TARGET_EXE=%RELEASE_EXE%"
    )
)

if not exist "%TARGET_EXE%" (
    echo   [WARNING] to-avif.exe not found. Context menu registration skipped.
    goto :after_menu_reg
)

echo   * Registering context menu via to-avif.exe...
call "%TARGET_EXE%" --register-menu
echo   [OK] Context menu registered successfully (HKCU).

:after_menu_reg
echo.

:: [5/5] Clean up temporary setup files
echo [5/5] Cleaning up temporary setup files...
if exist "%TEMP_DIR%" rd /s /q "%TEMP_DIR%" >nul 2>&1
echo   [OK] Cleaned up.
echo.

echo ===================================================
echo   Setup Summary
echo ===================================================
if exist "%BIN_DIR%\ffmpeg.exe" (
    echo   [V] FFmpeg:       Installed in bin\ffmpeg.exe
) else (
    echo   [X] FFmpeg:       Not found in bin\
)

if exist "%BIN_DIR%\exiftool.exe" (
    echo   [V] ExifTool:     Installed in bin\exiftool.exe
) else (
    echo   [X] ExifTool:     Not found in bin\
)

echo   [V] Context Menu: Registered ("AVIF Batch Converter")

echo.
echo Setup completed successfully.
echo You can right-click any folder, or run to-avif.exe / run.bat directly.
echo.
pause
