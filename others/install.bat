@echo off
setlocal

:: 获取当前脚本的目录
set "CURRENT_DIR=%~dp0"
set "FILENAME_7Z=vits-simple-api-windows-cpu-v0.6.16.7z"
set "FILENAME_ZIP=YuzuSoft_Vits.zip"
set "EXTRACT_DIR_MAIN=%CURRENT_DIR%vits-simple-api-windows-cpu-v0.6.16"
set "EXTRACT_DIR_MODEL=%EXTRACT_DIR_MAIN%\data\models\YuzuSoft_Vits"

echo Script directory: %CURRENT_DIR%
echo.

:: 下载-
echo Downloading %FILENAME_7Z%...
powershell -ExecutionPolicy Bypass -NoProfile -Command "Invoke-WebRequest -Uri 'https://github.com/Artrajz/vits-simple-api/releases/download/v0.6.16/vits-simple-api-windows-cpu-v0.6.16.7z' -OutFile '%CURRENT_DIR%%FILENAME_7Z%'"

if errorlevel 1 (
  echo Download of %FILENAME_7Z% failed!
  pause
  exit /b 1
)
echo Download of %FILENAME_7Z% successful.
echo.

:: 解压 vits-simple-api
echo Extracting %FILENAME_7Z% to %CURRENT_DIR%...
set "SEVENZIP_PATH="
if exist "%ProgramFiles%\7-Zip\7z.exe" (
    set "SEVENZIP_PATH=%ProgramFiles%\7-Zip\7z.exe"
) else if exist "%ProgramFiles(x86)%\7-Zip\7z.exe" (
    set "SEVENZIP_PATH=%ProgramFiles(x86)%\7-Zip\7z.exe"
) else (
    REM 
    where 7z >nul 2>nul
    if not errorlevel 1 (
        set "SEVENZIP_PATH=7z"
    )
)

if not defined SEVENZIP_PATH (
    echo 7-Zip (7z.exe) not found. Please install 7-Zip and add it to your PATH,
    echo or edit this script to provide the full path to 7z.exe.
    echo Cannot extract %FILENAME_7Z%.
    pause
    exit /b 1
)

echo Using 7-Zip: %SEVENZIP_PATH%
"%SEVENZIP_PATH%" x "%CURRENT_DIR%%FILENAME_7Z%" -o"%CURRENT_DIR%" -y > nul

if errorlevel 1 (
  echo Extraction of %FILENAME_7Z% failed! Check if 7-Zip is installed and working.
  pause
  exit /b 1
)
echo Extraction of %FILENAME_7Z% successful!
echo.

:: 下载YuzuSoft_Vits 模型
echo Downloading %FILENAME_ZIP%...
powershell -ExecutionPolicy Bypass -NoProfile -Command "Invoke-WebRequest -Uri 'https://github.com/Zao-chen/zao-chen.github.io/releases/download/%%E8%%B5%%84%%E6%%BA%%90%%E4%%B8%%8B%%E8%%BD%%BD/YuzuSoft_Vits.zip' -OutFile '%CURRENT_DIR%%FILENAME_ZIP%'"

if errorlevel 1 (
  echo Download of %FILENAME_ZIP% failed!
  pause
  exit /b 1
)
echo Download of %FILENAME_ZIP% successful.
echo.

:: 解压YuzuSoft_Vits模型
echo Ensuring model directory exists: %EXTRACT_DIR_MODEL%
if not exist "%EXTRACT_DIR_MODEL%" (
    mkdir "%EXTRACT_DIR_MODEL%"
    if errorlevel 1 (
        echo Failed to create directory: %EXTRACT_DIR_MODEL%
        pause
        exit /b 1
    )
    echo Directory created.
)

echo Extracting %FILENAME_ZIP% to %EXTRACT_DIR_MODEL%...
powershell -ExecutionPolicy Bypass -NoProfile -Command "Expand-Archive -Path '%CURRENT_DIR%%FILENAME_ZIP%' -DestinationPath '%EXTRACT_DIR_MODEL%' -Force"

if errorlevel 1 (
  echo Extraction of %FILENAME_ZIP% failed!
  pause
  exit /b 1
)
echo Extraction of %FILENAME_ZIP% successful!
echo.

:: 清理临时压缩文件 
echo Deleting temporary compressed files...
if exist "%CURRENT_DIR%%FILENAME_7Z%" del "%CURRENT_DIR%%FILENAME_7Z%"
if exist "%CURRENT_DIR%%FILENAME_ZIP%" del "%CURRENT_DIR%%FILENAME_ZIP%"
echo Deletion complete.
echo.

echo Installation complete!
pause
exit /b 0