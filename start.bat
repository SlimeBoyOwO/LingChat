@echo off
SETLOCAL ENABLEDELAYEDEXPANSION

:: 设置项目根目录为当前脚本所在目录
cd /d "%~dp0"
ECHO Current directory: %CD%

SET VENV_DIR=.venv
SET REQUIREMENTS_FILE=requirements.txt
SET BACKEND_SCRIPT=backend\webChat.py
SET FRONTEND_SCRIPT=server.js
SET BROWSER_URL=http://localhost:3000/

:: 1. 检查 .venv 目录是否存在
ECHO Checking for virtual environment (%VENV_DIR%)...
IF EXIST %VENV_DIR%\ (
    ECHO Found existing virtual environment.
    GOTO ActivateEnv
) ELSE (
    ECHO Virtual environment not found. Attempting to create one...
    GOTO CreateEnv
)
:CreateEnv
:: 检查 requirements.txt 是否存在
IF NOT EXIST %REQUIREMENTS_FILE% (
    ECHO ERROR: %REQUIREMENTS_FILE% not found in the current directory. Cannot create environment.
    PAUSE
    EXIT /B 1
)
:: 尝试查找并使用 Python 3.10, 3.11, 或 3.12 (优先 3.10)
SET PYTHON_EXE=
ECHO Searching for Python 3.10...
WHERE py -3.10 >nul 2>nul
IF %ERRORLEVEL% EQU 0 (
    SET PYTHON_EXE=py -3.10
    ECHO Found Python 3.10 via 'py' launcher.
    GOTO FoundPython
)
ECHO Python 3.10 not found via 'py'. Searching for Python 3.11...
WHERE py -3.11 >nul 2>nul
IF %ERRORLEVEL% EQU 0 (
    SET PYTHON_EXE=py -3.11
    ECHO Found Python 3.11 via 'py' launcher.
    GOTO FoundPython
)
ECHO Python 3.11 not found via 'py'. Searching for Python 3.12...
WHERE py -3.12 >nul 2>nul
IF %ERRORLEVEL% EQU 0 (
    SET PYTHON_EXE=py -3.12
    ECHO Found Python 3.12 via 'py' launcher.
    GOTO FoundPython
)
ECHO ERROR: Could not find Python 3.10, 3.11, or 3.12 using the 'py' launcher.
ECHO Please ensure Python 3.10-3.12 is installed and the 'py.exe' launcher is in your PATH.
PAUSE
EXIT /B 1
:FoundPython
ECHO Creating virtual environment using %PYTHON_EXE%...
%PYTHON_EXE% -m venv %VENV_DIR%
IF %ERRORLEVEL% NEQ 0 (
    ECHO ERROR: Failed to create virtual environment using %PYTHON_EXE%.
    PAUSE
    EXIT /B 1
)
ECHO Virtual environment created successfully.
:: 激活新环境并安装依赖
ECHO Activating virtual environment for dependency installation...
CALL %VENV_DIR%\Scripts\activate.bat
IF %ERRORLEVEL% NEQ 0 (
    ECHO ERROR: Failed to activate the new virtual environment.
    PAUSE
    EXIT /B 1
)
ECHO Installing dependencies from %REQUIREMENTS_FILE%...
pip install -r %REQUIREMENTS_FILE%
IF %ERRORLEVEL% NEQ 0 (
    ECHO ERROR: Failed to install dependencies using pip. Check %REQUIREMENTS_FILE% and your internet connection.
    ECHO The virtual environment window will remain open. Check for errors above.
    :: Keep the venv active in the current window for debugging if install fails
    cmd /k
    EXIT /B 1
)
ECHO Dependencies installed successfully.
GOTO RunBackend

:ActivateEnv
:: 激活现有环境

ECHO Activating existing virtual environment...
CALL %VENV_DIR%\Scripts\activate.bat
IF %ERRORLEVEL% NEQ 0 (
    ECHO ERROR: Failed to activate the existing virtual environment. Check if it's corrupted.
    PAUSE
    EXIT /B 1
)
ECHO Virtual environment activated.

:RunBackend
:: 2. 运行后端 Python 脚本 (在单独的窗口中，保持打开)
ECHO Starting Python backend (%BACKEND_SCRIPT%)...
IF NOT EXIST %BACKEND_SCRIPT% (
   ECHO ERROR: Backend script not found at %BACKEND_SCRIPT%
   PAUSE
   EXIT /B 1
)
ECHO Launching backend in a new window titled "LingChat Backend" (Window will stay open)...
:: Use cmd /k to keep the backend window open after the script finishes or errors
START "LingChat Backend" cmd /k "%VENV_DIR%\Scripts\python.exe %BACKEND_SCRIPT%"
ECHO Backend process started in a separate window. Waiting a few seconds...
TIMEOUT /T 5 /NOBREAK > NUL

:: 3. 运行前端 Node.js 脚本 (在单独的窗口中，保持打开)
ECHO Starting Node.js frontend (%FRONTEND_SCRIPT%)...
IF NOT EXIST %FRONTEND_SCRIPT% (
   ECHO ERROR: Frontend script not found at %FRONTEND_SCRIPT%
   PAUSE
   EXIT /B 1
)
:: 检查 node 是否可用
WHERE node >nul 2>nul
IF %ERRORLEVEL% NEQ 0 (
    ECHO ERROR: 'node' command not found. Please install Node.js and ensure it's in your PATH.
    PAUSE
    EXIT /B 1
)
ECHO Launching frontend in a new window titled "LingChat Frontend" (Window will stay open)...
START "LingChat Frontend" cmd /k "node %FRONTEND_SCRIPT%"
ECHO Frontend process started in a separate window. Waiting a few seconds...
TIMEOUT /T 3 /NOBREAK > NUL

:: 4. 打开浏览器
ECHO Opening browser to %BROWSER_URL% ...
START "" "%BROWSER_URL%"

ECHO.
ECHO ==================================================================
ECHO LingChat startup sequence initiated.
ECHO - Backend should be running in a window titled "LingChat Backend" (this window stays open).
ECHO - Frontend should be running in a window titled "LingChat Frontend" (this window stays open).
ECHO Check those windows for logs and errors.
ECHO ==================================================================
ECHO.
ECHO This launcher window will now pause. Press any key to close it.
ECHO Closing this window will NOT stop the Backend or Frontend windows.
PAUSE

ENDLOCAL
EXIT /B 0