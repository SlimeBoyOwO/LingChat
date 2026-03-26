from fastapi import APIRouter
import subprocess
import sys

router = APIRouter()


def _process_command_result(result: subprocess.CompletedProcess) -> dict:
    """处理子进程命令的执行结果，返回统一格式"""
    file_path = result.stdout.strip()
    if file_path and result.returncode == 0:
        return {"path": file_path}
    return {"path": ""}


@router.get("/api/settings/select-file")
async def select_file():
    """
    跨平台系统原生文件选择对话框
    支持 Windows、macOS 和 Linux
    返回选中的文件路径（绝对路径）
    """
    try:
        if sys.platform == 'win32':
            # ===== Windows: PowerShell =====
            script = """
Add-Type -AssemblyName System.Windows.Forms
$FileBrowser = New-Object System.Windows.Forms.OpenFileDialog
$FileBrowser.Filter = '可执行文件 (*.exe;*.bat)|*.exe;*.bat|所有文件 (*.*)|*.*'
$FileBrowser.Title = '选择语音合成软件'
$FileBrowser.Multiselect = $false
$null = $FileBrowser.ShowDialog()
Write-Output $FileBrowser.FileName
"""
            result = subprocess.run(
                ['powershell', '-Command', script],
                capture_output=True,
                text=True,
                timeout=60
            )
            return _process_command_result(result)
                
        elif sys.platform == 'darwin':
            # ===== macOS: AppleScript =====
            script = '''
tell application "Finder"
    set theFile to choose file with prompt "选择语音合成软件" of type {"public.executable", "public.shell-script"}
    return POSIX path of theFile
end tell
'''
            result = subprocess.run(
                ['osascript', '-e', script],
                capture_output=True,
                text=True,
                timeout=60
            )
            return _process_command_result(result)
                
        else:
            # ===== Linux: 依次尝试常见工具 =====
            commands = [
                (['zenity', '--file-selection', '--title=选择语音合成软件'], 'zenity'),
                (['kdialog', '--getopenfilename', '.', 'Executable files (*.sh *.bin)|All files (*)'], 'kdialog')
            ]
            
            for cmd, name in commands:
                try:
                    result = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
                    if result.returncode == 0:
                        return _process_command_result(result)
                except FileNotFoundError:
                    continue  # 工具未安装，尝试下一个
            
            # 所有工具都不可用，优雅降级
            return {
                "path": "",
                "error": "未找到文件选择工具（需要 zenity 或 kdialog），请手动输入路径"
            }
            
    except subprocess.TimeoutExpired:
        return {"path": "", "error": "操作超时（60秒）"}
    except Exception as e:
        return {"path": "", "error": f"文件选择失败: {str(e)}"}
