@echo off
setlocal enabledelayedexpansion

echo Looking for virtual environment...

:: Define possible venv dirs
set "VENV_DIRS=.venv venv env virtualenv"

:: Roll them all
for %%d in (%VENV_DIRS%) do (
    if exist "%%d\Scripts\activate.bat" (
        set "FOUND_VENV=%%d"
        goto :activate_venv
    )
)

:: If virtual env not found
echo Error: virtual env was not found!
echo Possible virtual env dirs: %VENV_DIRS%
echo Ensure that it is created and placed under this dir
pause
exit /b 1

:activate_venv
echo Venv found: !FOUND_VENV!
call "!FOUND_VENV!\Scripts\activate.bat"

if %errorlevel% neq 0 (
    echo Error, activate failed
    pause
    exit /b 1
)

echo Venv was activated successfully!

:: Start frontend in a new window
echo Starting frontend...
start "Frontend Server" cmd /k "cd frontend_vue && start.bat"

:: Main loop for running Python program
:restart_python
echo Running Python program...
python main.py

:: Check program status
if %errorlevel% neq 0 (
    echo Error: runtime error, code: %errorlevel%
    echo Program will restart in 3 seconds...
) else (
    echo Python program completed successfully!
    echo Program will restart in 3 seconds...
)

:: Wait 3 seconds before restarting
timeout /t 3 /nobreak >nul

:: Restart the Python program
goto :restart_python

pause