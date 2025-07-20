@echo off
cls
rem =========================================================================
rem         LingChat Desktop Pet Launcher (run_pet.bat)
rem =========================================================================
rem
rem Purpose:
rem 1. Sets the PYTHONUTF8=1 environment variable to force Python into
rem    UTF-8 mode, preventing common encoding errors with non-ASCII characters.
rem 2. Automatically locates and uses the Python interpreter from the .venv
rem    virtual environment in the project root.
rem 3. Runs the main desktop pet script.
rem
rem How to use:
rem - Place this file in the project root directory (e.g., /develop).
rem - Simply double-click this file to run the application.
rem
rem =========================================================================

echo [LingChat Pet] Preparing launch environment...

rem --- Step 1: Force Python to use UTF-8 ---
set PYTHONUTF8=1
echo [LingChat Pet] Environment variable set: PYTHONUTF8=%PYTHONUTF8%

rem --- Step 2: Define Paths ---
rem Get the directory where this batch file is located (the project root)
set "PROJECT_ROOT=%~dp0"
echo [LingChat Pet] Project root detected: %PROJECT_ROOT%

rem Define the full paths to the Python executable and the main script
set "VENV_PYTHON=%PROJECT_ROOT%.venv\Scripts\python.exe"
set "PET_SCRIPT=%PROJECT_ROOT%backend\desktop_pet\desktop_pet.py"

rem --- Step 3: Validate Paths ---
echo [LingChat Pet] Verifying required files...

rem Check if the virtual environment's Python executable exists
if not exist "%VENV_PYTHON%" (
    echo.
    echo [FATAL ERROR] Python interpreter not found in the virtual environment.
    echo Looked for: "%VENV_PYTHON%"
    echo.
    echo Please ensure a virtual environment named ".venv" has been created
    echo in the project root directory.
    echo.
    pause
    exit /b 1
)

rem Check if the main pet script exists
if not exist "%PET_SCRIPT%" (
    echo.
    echo [FATAL ERROR] The main desktop pet script was not found.
    echo Looked for: "%PET_SCRIPT%"
    echo.
    echo Please ensure the script exists at the expected path.
    echo.
    pause
    exit /b 1
)

echo [LingChat Pet] Verification successful.
echo.
echo =======================================================
echo [LingChat Pet] Launching the application...
echo =======================================================
echo.

rem --- Step 4: Run the Application ---
rem Execute the script using the Python from our virtual environment.
rem The console will now show output from the Python script.
"%VENV_PYTHON%" "%PET_SCRIPT%"

echo.
echo =======================================================
echo [LingChat Pet] Program has exited.
echo Press any key to close this window.
echo =======================================================
pause > nul