@echo off
SETLOCAL ENABLEDELAYEDEXPANSION

cd /d "%~dp0"
ECHO Current directory: %CD%

SET VENV_DIR=.venv
SET REQUIREMENTS_FILE=requirements.txt
SET BACKEND_SCRIPT=backend\webChat.windows.py
SET FRONTEND_SCRIPT=frontend\server.js
SET BROWSER_URL=http://localhost:3000/

ECHO Checking for virtual environment (%VENV_DIR%)...
IF EXIST %VENV_DIR%\ (
    ECHO Found existing virtual environment.
    GOTO ActivateEnv
) ELSE (
    ECHO Virtual environment not found. Attempting to create one...
    GOTO CreateEnv
)
:CreateEnv

IF NOT EXIST %REQUIREMENTS_FILE% (
    ECHO ERROR: %REQUIREMENTS_FILE% not found in the current directory. Cannot create environment.
    PAUSE
    EXIT /B 1
)

SET PYTHON_EXE=
ECHO Searching for Python 3.12...
WHERE py -3.12 >nul 2>nul
IF %ERRORLEVEL% EQU 0 (
    SET PYTHON_EXE=py -3.12
    ECHO Found Python 3.12 via 'py' launcher.
    GOTO FoundPython
)
ECHO Python 3.12 not found via 'py'. Searching for Python 3.11...
WHERE py -3.11 >nul 2>nul
IF %ERRORLEVEL% EQU 0 (
    SET PYTHON_EXE=py -3.11
    ECHO Found Python 3.11 via 'py' launcher.
    GOTO FoundPython
)
ECHO Python 3.11 not found via 'py'. Searching for Python 3.10...
WHERE py -3.10 >nul 2>nul
IF %ERRORLEVEL% EQU 0 (
    SET PYTHON_EXE=py -3.10
    ECHO Found Python 3.10 via 'py' launcher.
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

ECHO Activating existing virtual environment...
CALL %VENV_DIR%\Scripts\activate.bat
IF %ERRORLEVEL% NEQ 0 (
    ECHO ERROR: Failed to activate the existing virtual environment. Check if it's corrupted.
    PAUSE
    EXIT /B 1
)
ECHO Virtual environment activated.

:RunBackend
ECHO Starting Python backend (%BACKEND_SCRIPT%)...
IF NOT EXIST %BACKEND_SCRIPT% (
   ECHO ERROR: Backend script not found at %BACKEND_SCRIPT%
   PAUSE
   EXIT /B 1
)
ECHO Launching backend in a new window titled "LingChat" (Window will stay open)...
:: Use cmd /k to keep the backend window open after the script finishes or errors
START "LingChat" cmd /k "%VENV_DIR%\Scripts\python.exe %BACKEND_SCRIPT%"
ECHO Backend process started in a separate window. Waiting a few seconds...
TIMEOUT /T 5 /NOBREAK > NUL
