@echo off
SETLOCAL ENABLEDELAYEDEXPANSION

cd /d "%~dp0" > nul 2>&1

SET VENV_DIR=.venv
SET REQUIREMENTS_FILE=requirements.txt
SET BACKEND_SCRIPT=backend\windows_main.py
SET FRONTEND_SCRIPT=frontend\server.js
SET BROWSER_URL=http://localhost:3000/

IF EXIST %VENV_DIR%\ (
    GOTO ActivateEnv
) ELSE (
    GOTO CreateEnv
)
:CreateEnv

IF NOT EXIST %REQUIREMENTS_FILE% (
    ECHO ERROR: %REQUIREMENTS_FILE% not found in the current directory. Cannot create environment.
    PAUSE
    EXIT /B 1
)

SET PYTHON_EXE=
WHERE py -3.12 >nul 2>nul
IF %ERRORLEVEL% EQU 0 (
    SET PYTHON_EXE=py -3.12
    GOTO FoundPython
)
WHERE py -3.11 >nul 2>nul
IF %ERRORLEVEL% EQU 0 (
    SET PYTHON_EXE=py -3.11
    GOTO FoundPython
)
WHERE py -3.10 >nul 2>nul
IF %ERRORLEVEL% EQU 0 (
    SET PYTHON_EXE=py -3.10
    GOTO FoundPython
)
ECHO ERROR: Could not find Python 3.10, 3.11, or 3.12 using the 'py' launcher.
ECHO Please ensure Python 3.10-3.12 is installed and the 'py.exe' launcher is in your PATH.
PAUSE
EXIT /B 1
:FoundPython
%PYTHON_EXE% -m venv %VENV_DIR% > nul 2>&1
IF %ERRORLEVEL% NEQ 0 (
    ECHO ERROR: Failed to create virtual environment using %PYTHON_EXE%.
    PAUSE
    EXIT /B 1
)

CALL %VENV_DIR%\Scripts\activate.bat > nul 2>&1
IF %ERRORLEVEL% NEQ 0 (
    ECHO ERROR: Failed to activate the new virtual environment.
    PAUSE
    EXIT /B 1
)
pip install -r %REQUIREMENTS_FILE% > nul 2>&1
IF %ERRORLEVEL% NEQ 0 (
    ECHO ERROR: Failed to install dependencies using pip. Check %REQUIREMENTS_FILE% and your internet connection.
    ECHO The virtual environment window will remain open. Check for errors above.
    cmd /k
    EXIT /B 1
)
GOTO RunBackend

:ActivateEnv

CALL %VENV_DIR%\Scripts\activate.bat > nul 2>&1
IF %ERRORLEVEL% NEQ 0 (
    ECHO ERROR: Failed to activate the existing virtual environment. Check if it's corrupted.
    PAUSE
    EXIT /B 1
)

:RunBackend
IF NOT EXIST %BACKEND_SCRIPT% (
   ECHO ERROR: Backend script not found at %BACKEND_SCRIPT%
   PAUSE
   EXIT /B 1
)
START "LingChat" cmd /k "%VENV_DIR%\Scripts\python.exe %BACKEND_SCRIPT%"
TIMEOUT /T 5 /NOBREAK > NUL
