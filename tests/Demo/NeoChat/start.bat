@echo off
rem 切换到当前批处理文件所在的目录，确保相对路径正确
cd /d "%~dp0"

rem 使用.venv中的python解释器执行main.py
rem @echo off会阻止这一行命令本身被输出，但main.py的输出会正常显示
.venv\Scripts\python.exe main.py

rem 可选：如果您希望脚本运行完毕后窗口不停留，可以删除下一行
rem pause