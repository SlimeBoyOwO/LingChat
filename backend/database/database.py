import sqlite3
from datetime import datetime

DB_NAME = "chat_system.db"


def init_db():
    conn = sqlite3.connect(DB_NAME)
    cursor = conn.cursor()

    # 启用外键支持
    cursor.execute("PRAGMA foreign_keys = ON")

    # 创建用户表
    cursor.execute("""
    CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        username TEXT UNIQUE NOT NULL,
        password TEXT NOT NULL
    )
    """)

    # 创建对话表
    cursor.execute("""
    CREATE TABLE IF NOT EXISTS conversations (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        main_character_name TEXT NOT NULL,
        main_character_school TEXT,
        ai_name TEXT NOT NULL,
        ai_school TEXT,
        prompt TEXT,
        json_data TEXT
    )
    """)

    # 创建用户-对话关联表
    cursor.execute("""
    CREATE TABLE IF NOT EXISTS user_conversations (
        user_id INTEGER NOT NULL,
        conversation_id INTEGER NOT NULL,
        timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (user_id, conversation_id),
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
        FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
    )
    """)

    # 创建索引提高查询性能
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_user_conv ON user_conversations(user_id)")
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_conv_time ON user_conversations(timestamp)")

    conn.commit()
    conn.close()


def get_db_connection():
    conn = sqlite3.connect(DB_NAME)
    conn.row_factory = sqlite3.Row  # 允许以字典方式访问结果
    return conn