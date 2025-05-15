from pathlib import Path
from platformdirs import user_data_dir

# 应用信息（用于构建平台特定路径）
APP_NAME = "py_ling_chat"
APP_AUTHOR = "py_ling_chat"  # Windows 用于 AppData\Roaming\MyCompany\MyApp

# 获取用户数据目录路径
root_path = Path(user_data_dir(appname=APP_NAME, appauthor=APP_AUTHOR))

# 确保路径存在
root_path.mkdir(parents=True, exist_ok=True)

