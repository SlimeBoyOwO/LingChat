from database import get_db_connection
import json

class UserModel:
    @staticmethod
    def create_user(username, password):
        conn = get_db_connection()
        cursor = conn.cursor()
        cursor.execute(
            "INSERT INTO users (username, password) VALUES (?, ?)",
            (username, password)
        )
        user_id = cursor.lastrowid
        conn.commit()
        conn.close()
        return user_id

    @staticmethod
    def get_user_by_id(user_id):
        conn = get_db_connection()
        user = conn.execute(
            "SELECT * FROM users WHERE id = ?", 
            (user_id,)
        ).fetchone()
        conn.close()
        return dict(user) if user else None

class ConversationModel:
    @staticmethod
    def create_conversation(main_character, ai_name, prompt=None, json_data=None, **kwargs):
        conn = get_db_connection()
        cursor = conn.cursor()
        cursor.execute(
            """INSERT INTO conversations 
            (main_character_name, main_character_school, ai_name, ai_school, prompt, json_data)
            VALUES (?, ?, ?, ?, ?, ?)""",
            (
                main_character['name'],
                main_character.get('school'),
                ai_name['name'],
                ai_name.get('school'),
                prompt,
                json.dumps(json_data) if json_data else None
            )
        )
        conv_id = cursor.lastrowid
        conn.commit()
        conn.close()
        return conv_id

    @staticmethod
    def get_conversation(conversation_id):
        conn = get_db_connection()
        conv = conn.execute(
            "SELECT * FROM conversations WHERE id = ?", 
            (conversation_id,)
        ).fetchone()
        conn.close()
        return dict(conv) if conv else None

class UserConversationModel:
    @staticmethod
    def link_user_conversation(user_id, conversation_id):
        conn = get_db_connection()
        cursor = conn.cursor()
        cursor.execute(
            """INSERT INTO user_conversations 
            (user_id, conversation_id) VALUES (?, ?)""",
            (user_id, conversation_id)
        )
        conn.commit()
        conn.close()
        return True

    @staticmethod
    def get_user_conversations(user_id, limit=10, offset=0):
        conn = get_db_connection()
        conversations = conn.execute(
            """SELECT c.*, uc.timestamp 
            FROM conversations c
            JOIN user_conversations uc ON c.id = uc.conversation_id
            WHERE uc.user_id = ?
            ORDER BY uc.timestamp DESC
            LIMIT ? OFFSET ?""",
            (user_id, limit, offset)
        ).fetchall()
        conn.close()
        return [dict(conv) for conv in conversations]