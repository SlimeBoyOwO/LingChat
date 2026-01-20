import os  # 添加os模块用于路径操作
import sqlite3
from enum import Enum

from ling_chat.utils.runtime_path import user_data_path

# 修改数据库路径到data目录
DATA_DIR = user_data_path
DB_NAME = os.path.join(DATA_DIR, "chat_system.db")  # 使用os.path.join确保跨平台兼容性


class Role(Enum):
    SYSTEM = "system"
    USER = "user"
    ASSISTANT = "assistant"


def init_db():
    # 确保data目录存在
    if not os.path.exists(DATA_DIR):
        os.makedirs(DATA_DIR)

    conn = sqlite3.connect(DB_NAME)
    cursor = conn.cursor()

    # 启用外键支持
    cursor.execute("PRAGMA foreign_keys = ON")

    # 创建用户表
    cursor.execute("""
    CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        username TEXT UNIQUE NOT NULL,
        password TEXT NOT NULL,
        last_chat_character INTEGER DEFAULT 1,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (last_chat_character) REFERENCES characters(id) ON DELETE SET DEFAULT
    )
    """)

    # 创建对话表
    cursor.execute("""
    CREATE TABLE IF NOT EXISTS conversations (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        title TEXT NOT NULL DEFAULT 'New Conversation',
        last_message_id INTEGER,
        owned_user INTEGER NOT NULL,
        character INTEGER,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (character) REFERENCES characters(id) ON DELETE CASCADE,
        FOREIGN KEY (owned_user) REFERENCES users(id) ON DELETE CASCADE,
        FOREIGN KEY (last_message_id) REFERENCES messages(id) ON DELETE SET NULL
    )
    """)

    cursor.execute("""
    CREATE TABLE IF NOT EXISTS characters (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        title TEXT NOT NULL DEFAULT '默认',
        resource_path TEXT NOT NULL DEFAULT 'game_data/characters/default'
    )           
    """)

    # 创建消息表
    cursor.execute("""
    CREATE TABLE IF NOT EXISTS messages (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        role TEXT NOT NULL CHECK(role IN ('system', 'user', 'assistant')),
        content TEXT NOT NULL,
        owned_conversation INTEGER NOT NULL,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (owned_conversation) REFERENCES conversations(id) ON DELETE CASCADE
    )
    """)

    # 创建消息关系表（替代原来的parent_message_ids）
    cursor.execute("""
    CREATE TABLE IF NOT EXISTS message_relations (
        parent_id INTEGER NOT NULL,
        child_id INTEGER NOT NULL,
        PRIMARY KEY (parent_id, child_id),
        FOREIGN KEY (parent_id) REFERENCES messages(id) ON DELETE CASCADE,
        FOREIGN KEY (child_id) REFERENCES messages(id) ON DELETE CASCADE
    )
    """)

    # 创建索引提高查询性能
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_conversation_user ON conversations(owned_user)")
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_message_conversation ON messages(owned_conversation)")
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_message_relations_parent ON message_relations(parent_id)")
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_message_relations_child ON message_relations(child_id)")

    conn.commit()
    conn.close()


def get_db_connection():
    # 确保data目录存在
    if not os.path.exists(DATA_DIR):
        os.makedirs(DATA_DIR)

    conn = sqlite3.connect(DB_NAME)
    conn.row_factory = sqlite3.Row  # 允许以字典方式访问结果
    return conn


def main():
    # 初始化数据库
    init_db()

    # 测试数据库连接和表结构
    conn = get_db_connection()
    cursor = conn.cursor()

    # 检查表是否存在
    cursor.execute("SELECT name FROM sqlite_master WHERE type='table'")
    tables = cursor.fetchall()
    print("数据库中的表:")
    for table in tables:
        print(table['name'])

    # 显示每个表的结构
    for table in tables:
        print(f"\n{table['name']}表结构:")
        cursor.execute(f"PRAGMA table_info({table['name']})")
        columns = cursor.fetchall()
        for col in columns:
            print(f"{col['name']}: {col['type']}")

    conn.close()
    print(f"\n数据库初始化完成，数据库文件位置: {os.path.abspath(DB_NAME)}")
    print("表结构验证通过。")


if __name__ == "__main__":
    main()
