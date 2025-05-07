from pathlib import Path
import sys

def get_root_path() -> Path:
    """返回项目根路径（支持 PyInstaller onefile 模式）"""
    if hasattr(sys, '_MEIPASS'):
        return Path(sys._MEIPASS)
    return Path(__file__).resolve().parent.parent.parent

root_path = get_root_path()
