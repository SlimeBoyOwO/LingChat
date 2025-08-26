@echo off
setlocal

cd %~dp0
cd ../
call .\.venv\Scripts\activate.bat
call python .\main.py

endlocal