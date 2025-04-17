@echo off

:: 获取当前脚本的目录
set CURRENT_DIR=%~dp0

:: Download the file from GitHub
powershell -Command "Invoke-WebRequest -Uri [https://github.com/Artrajz/vits-simple-api/releases/download/v0.6.16/vits-simple-api-windows-cpu-v0.6.16.7z](https://github.com/Artrajz/vits-simple-api/releases/download/v0.6.16/vits-simple-api-windows-cpu-v0.6.16.7z) -OutFile %CURRENT_DIR%vits-simple-api-windows-cpu-v0.6.16.7z"

:: Extract the file to the third_party directory
tar -xvf %CURRENT_DIR%vits-simple-api-windows-cpu-v0.6.16.7z -C %CURRENT_DIR%

:: Check if the extraction was successful
if %errorlevel% == 0 (
  echo Extraction successful!
) else (
  echo Extraction failed!
  pause
  exit /b
)

:: Download and extract another file
powershell -Command "Invoke-WebRequest -Uri [https://github.com/Zao-chen/zao-chen.github.io/releases/download/%E8%B5%84%E6%BA%90%E4%B8%8B%E8%BD%BD/YuzuSoft_Vits.zip](https://github.com/Zao-chen/zao-chen.github.io/releases/download/%E8%B5%84%E6%BA%90%E4%B8%8B%E8%BD%BD/YuzuSoft_Vits.zip) -OutFile %CURRENT_DIR%YuzuSoft_Vits.zip"
tar -xvf %CURRENT_DIR%YuzuSoft_Vits.zip -C %CURRENT_DIR%\vits-simple-api-windows-cpu-v0.6.16\data\models\YuzuSoft_Vits

:: Check if the extraction was successful
if %errorlevel% == 0 (
  echo Extraction successful!
) else (
  echo Extraction failed!
  pause
  exit /b
)

:: Delete the temporary compressed file
del %CURRENT_DIR%vits-simple-api-windows-cpu-v0.6.16.7z
del %CURRENT_DIR%YuzuSoft_Vits.zip

echo Installation complete!
pause
exit
