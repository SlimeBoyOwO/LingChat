@echo off
setlocal enabledelayedexpansion

echo Looking for virtual enviroment...

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
echo Ensure that it is fucking created and placed under this dir
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
echo Running baka program...

:: Run Python main program
python main.py

:: Check grogram status
if %errorlevel% neq 0 (
    echo Error: runtime error, code: %errorlevel%
) else (
    echo Python program completed successfully!
)

pause