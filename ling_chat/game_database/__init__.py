"""
game_database 包初始化。

注意：不要在 import 时自动初始化/建库。
原因：
- import 发生在非常多的场景（例如 pytest 收集测试、类型检查、只读工具等）
- 自动建库会触发文件系统写入与数据库连接，在 CI/只读环境中容易失败

正确姿势：在应用启动生命周期中显式调用 `ling_chat.game_database.database.init_db()`，
例如 `ling_chat/api/app_server.py` 的 lifespan。
"""

from ling_chat.game_database.database import init_db  # noqa: F401