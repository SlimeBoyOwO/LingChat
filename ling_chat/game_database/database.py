
import os
from sqlmodel import SQLModel, create_engine
from ling_chat.utils.runtime_path import user_data_path

# 修改数据库路径到data目录
DATA_DIR = user_data_path
DB_NAME = os.path.join(DATA_DIR, "game_database.db")  # 使用os.path.join确保跨平台兼容性

sqlite_url = f"sqlite:///{DB_NAME}"

# 创建引擎
engine = create_engine(sqlite_url, echo=False)  # echo=True 会打印 SQL 语句，方便调试


# ==========================================
# 初始化函数
# ==========================================

def create_db_and_tables():
    """初始化数据库表结构"""
    # Import models to ensure they are registered with SQLModel.metadata
    from ling_chat.game_database import models  # noqa: F401
    
    SQLModel.metadata.create_all(engine)
    print(f"数据库 {DB_NAME} 初始化成功！")

def init_db():
    create_db_and_tables()