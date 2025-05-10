from py_ling_chat.database.database import get_db_connection
from typing import Optional, List, Dict
import hashlib


class UserModel:
    @staticmethod
    def create_user(username: str, password: str) -> Optional[int]:
        """
        创建新用户，用户名必须唯一。
        """
        conn = get_db_connection()
        cursor = conn.cursor()

        # 检查用户名是否存在
        cursor.execute("SELECT id FROM users WHERE username = ?", (username,))
        if cursor.fetchone():
            conn.close()
            raise ValueError(f"用户名 '{username}' 已存在")

        # 暂时懒得加盐了，测试一下看看对不对
        hashed_password = password

        cursor.execute(
            "INSERT INTO users (username, password) VALUES (?, ?)",
            (username, hashed_password)
        )
        user_id = cursor.lastrowid
        conn.commit()
        conn.close()
        return user_id

    @staticmethod
    def get_user_by_id(user_id: int) -> Optional[Dict]:
        conn = get_db_connection()
        cursor = conn.cursor()
        cursor.execute("SELECT * FROM users WHERE id = ?", (user_id,))
        row = cursor.fetchone()
        conn.close()
        return dict(row) if row else None


class UserConversationModel:
    @staticmethod
    def get_user_conversations(user_id: int, page: int = 1, page_size: int = 10) -> Dict:
        """
        分页获取用户的所有对话，按更新时间倒序排列
        返回 dict：包含 conversations 和 total 总数
        """
        offset = (page - 1) * page_size
        conn = get_db_connection()
        cursor = conn.cursor()

        # 获取总数
        cursor.execute("""
            SELECT COUNT(*) FROM conversations WHERE owned_user = ?
        """, (user_id,))
        total = cursor.fetchone()[0]

        # 获取分页数据
        cursor.execute("""
            SELECT id, title, updated_at, last_message_id, created_at
            FROM conversations
            WHERE owned_user = ?
            ORDER BY updated_at DESC
            LIMIT ? OFFSET ?
        """, (user_id, page_size, offset))
        conversations = cursor.fetchall()
        conn.close()

        return {
            "conversations": [dict(row) for row in conversations],
            "total": total
        }
