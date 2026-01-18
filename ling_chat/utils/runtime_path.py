import sys
import tempfile
from pathlib import Path

from platformdirs import user_data_dir


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
            return Path(sys.executable)
            # return Path(sys._MEIPASS) 这个地方会出现类型检查错误，@PL，之后查看一下
        # PyInstaller 目录模式（可执行文件所在目录）
        return Path(sys.executable).parent

    # 开发环境：基于当前文件的路径推导包根目录
    return Path(__file__).parent.parent  # 根据实际层级调整

def get_user_data_path() -> Path:
    """
    获取用户数据目录
    Returns:
        Path: 用户数据目录的 Path 对象
    """
    if getattr(sys, 'frozen', False):
        # PyInstaller 单文件模式
        if hasattr(sys, '_MEIPASS'):
            return Path(user_data_dir(appname=APP_NAME, appauthor=APP_AUTHOR))
    return get_package_root() / "data"  # 开发环境使用 package 根目录下的 data 文件夹

# 应用信息（用于构建平台特定路径）
APP_NAME = "ling_chat"
APP_AUTHOR = "ling_chat"  # Windows 用于 AppData\Roaming\MyCompany\MyApp

package_root: Path = get_package_root()

static_path: Path = package_root / "static"
third_party_path: Path = package_root / "third_party"  # 第三方资源目录
user_data_path: Path = get_user_data_path()  # 用户数据目录
temp_path: Path = Path(tempfile.mkdtemp(prefix="ling_chat"))  # 获取系统临时目录

__all__ = [
    "package_root",
    "static_path",
    "third_party_path",
    "user_data_path",
    "temp_path",
    "APP_NAME",
    "APP_AUTHOR"
]
