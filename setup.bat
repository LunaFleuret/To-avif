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

:: [1/5] Prepare directories
echo [1/5] Preparing directories...
if not exist "%BIN_DIR%" mkdir "%BIN_DIR%"
if not exist "%TEMP_DIR%" mkdir "%TEMP_DIR%"

:: Deploy release binary to root if present
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
if exist "%BIN_DIR%\ffmpeg.exe" (
    echo   * ffmpeg.exe: Found in bin\
) else (
    echo   * ffmpeg.exe: Missing. Downloading release package...
    set "FFMPEG_ZIP=%TEMP_DIR%\ffmpeg.zip"
    curl.exe -fL --progress-bar -o "!FFMPEG_ZIP!" "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip"
    if exist "!FFMPEG_ZIP!" (
        echo     Extracting ffmpeg to bin\...
        powershell -NoProfile -Command "Expand-Archive -Path '!FFMPEG_ZIP!' -DestinationPath '%TEMP_DIR%\ffmpeg_extracted' -Force"
        for /r "%TEMP_DIR%\ffmpeg_extracted" %%f in (ffmpeg.exe) do (
            if exist "%%f" copy /y "%%f" "%BIN_DIR%\" >nul
        )
        if exist "%BIN_DIR%\ffmpeg.exe" (
            echo     [OK] ffmpeg.exe installed successfully.
        ) else (
            echo     [WARNING] Could not extract ffmpeg.exe. Please place it manually in bin\.
        )
        del /f /q "!FFMPEG_ZIP!" >nul 2>&1
    ) else (
        echo     [WARNING] Failed to download FFmpeg.
    )
)
echo.

:: [3/5] Check and download ExifTool (Metadata engine & perl files)
echo [3/5] Checking ExifTool (Metadata preservation engine)...
set "EXIF_PERL_OK=0"
if exist "%BIN_DIR%\exiftool_files" set "EXIF_PERL_OK=1"
if exist "%BIN_DIR%\exiftool.exe" if "!EXIF_PERL_OK!"=="1" (
    echo   * ExifTool: Found in bin\
) else (
    echo   * ExifTool: Incomplete or missing. Downloading standalone package...
    set "EXIF_ZIP=%TEMP_DIR%\exiftool.zip"
    curl.exe -fL --progress-bar -o "!EXIF_ZIP!" "https://sourceforge.net/projects/exiftool/files/latest/download"
    if exist "!EXIF_ZIP!" (
        echo     Extracting ExifTool package...
        powershell -NoProfile -Command "Expand-Archive -Path '!EXIF_ZIP!' -DestinationPath '%TEMP_DIR%\exif_extracted' -Force"
        for /r "%TEMP_DIR%\exif_extracted" %%f in (exiftool*.exe) do (
            if exist "%%f" copy /y "%%f" "%BIN_DIR%\exiftool.exe" >nul
        )
        for /d /r "%TEMP_DIR%\exif_extracted" %%d in (exiftool_files) do (
            if exist "%%d" (
                robocopy "%%d" "%BIN_DIR%\exiftool_files" /e /np /nfl /ndl >nul 2>&1
            )
        )
        if exist "%BIN_DIR%\exiftool.exe" (
            echo     [OK] ExifTool installed successfully.
        ) else (
            echo     [WARNING] Could not extract ExifTool. Please place it manually in bin\.
        )
        del /f /q "!EXIF_ZIP!" >nul 2>&1
    ) else (
        echo     [WARNING] Failed to download ExifTool.
    )
)
echo.

:: [4/5] Register Windows Explorer Context Menu
echo [4/5] Registering Windows Context Menu...
set "TARGET_EXE=%ROOT_EXE%"
if not exist "%TARGET_EXE%" (
    if exist "%RELEASE_EXE%" set "TARGET_EXE=%RELEASE_EXE%"
)

set "MENU_REG_STATUS=Skipped (to-avif.exe not found)"
if exist "%TARGET_EXE%" (
    call "%TARGET_EXE%" --register-menu
    set "MENU_REG_STATUS=Registered (AVIF Batch Converter)"
    echo   [OK] Context menu registered successfully.
) else (
    echo   [INFO] to-avif.exe not found yet. It will automatically register on first launch.
)
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

echo   [*] Context Menu: %MENU_REG_STATUS%
echo ===================================================
echo.
pause
