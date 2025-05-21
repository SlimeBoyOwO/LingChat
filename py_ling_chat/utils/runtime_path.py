from pathlib import Path
from platformdirs import user_data_dir
import sys
import os
import tempfile


def get_package_root() -> Path:
    """
    获取当前 package 的根目录（兼容 PyInstaller 和开发环境）
    Returns:
        Path: 根目录的 Path 对象
    """
    # 判断是否在 PyInstaller 打包环境中运行
    if getattr(sys, 'frozen', False):
        # PyInstaller 单文件模式（临时解压目录）
        if hasattr(sys, '_MEIPASS'):
            return Path(sys._MEIPASS)
        # PyInstaller 目录模式（可执行文件所在目录）
        return Path(sys.executable).parent

    # 开发环境：基于当前文件的路径推导包根目录
    return Path(__file__).parent.parent  # 根据实际层级调整

# 应用信息（用于构建平台特定路径）
APP_NAME = "py_ling_chat"
APP_AUTHOR = "py_ling_chat"  # Windows 用于 AppData\Roaming\MyCompany\MyApp

package_root: Path = get_package_root()

static_path: Path = package_root / "static"
user_data_path: Path = Path(user_data_dir(appname=APP_NAME, appauthor=APP_AUTHOR))
temp_path: Path = Path(tempfile.gettempdir())  # 获取系统临时目录
