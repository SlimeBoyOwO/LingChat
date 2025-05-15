from typing import List, Dict, Optional
from .database import get_db_connection, Role
import json


class ConversationModel:
    @staticmethod
    def create_conversation(user_id: int, title: str = "New Conversation") -> int:
        """创建新对话，返回 conversation_id"""
        conn = get_db_connection()
        cursor = conn.cursor()

        cursor.execute(
            "INSERT INTO conversations (title, owned_user) VALUES (?, ?)",
            (title, user_id)
        )
        conv_id = cursor.lastrowid

        conn.commit()
        conn.close()
        return conv_id

    @staticmethod
    def load_conversation(user_id: int, messages: List[Dict[str, str]], title: Optional[str] = None) -> int:
        """
        将完整对话插入数据库
        :param user_id: 所属用户ID
        :param messages: 消息列表，每条为{"role": "user/assistant/system", "content": "..."}
        :param title: 可选对话标题
        :return: conversation_id
        """
        if not messages:
            raise ValueError("消息列表不能为空")

        # 自动生成标题
        if not title:
            first_user_msg = next((m for m in messages if m["role"] == Role.USER.value), None)
            title = (first_user_msg["content"][:20] + "...") if first_user_msg else "New Conversation"

        conn = get_db_connection()
        cursor = conn.cursor()

        try:
            # 创建 conversation
            cursor.execute(
                "INSERT INTO conversations (title, owned_user) VALUES (?, ?)",
                (title, user_id)
            )
            conversation_id = cursor.lastrowid

            prev_msg_id = None
            for msg in messages:
                # 插入消息
                cursor.execute(
                    "INSERT INTO messages (role, content, owned_conversation) VALUES (?, ?, ?)",
                    (msg["role"], msg["content"], conversation_id)
                )
                current_msg_id = cursor.lastrowid

                # 插入 message_relations
                if prev_msg_id is not None:
                    cursor.execute(
                        "INSERT INTO message_relations (parent_id, child_id) VALUES (?, ?)",
                        (prev_msg_id, current_msg_id)
                    )

                prev_msg_id = current_msg_id

            # 更新 conversation 的 last_message_id 和更新时间
            cursor.execute(
                "UPDATE conversations SET last_message_id = ?, updated_at = datetime('now') WHERE id = ?",
                (prev_msg_id, conversation_id)
            )

            conn.commit()
            return conversation_id

        except Exception as e:
            conn.rollback()
            raise e
        finally:
            conn.close()

    @staticmethod
    def append_messages_to_conversation(conversation_id: int, messages: List[Dict[str, str]]) -> None:
        """
        向已有对话中追加新消息（自动建立父子关系、更新最后消息 ID）
        """
        if not messages:
            raise ValueError("追加的消息列表不能为空")

        conn = get_db_connection()
        cursor = conn.cursor()

        try:
            # 获取当前对话存在性和最后一条消息 ID
            cursor.execute("SELECT last_message_id FROM conversations WHERE id = ?", (conversation_id,))
            row = cursor.fetchone()
            if not row:
                raise ValueError(f"对话 ID {conversation_id} 不存在")
            prev_msg_id = row["last_message_id"]

            last_msg_id = None

            for msg in messages:
                role = msg["role"]
                content = msg["content"]

                # 插入消息
                cursor.execute(
                    "INSERT INTO messages (role, content, owned_conversation) VALUES (?, ?, ?)",
                    (role, content, conversation_id)
                )
                current_msg_id = cursor.lastrowid

                # 建立父子关系（如果不是第一条追加）
                if prev_msg_id:
                    cursor.execute(
                        "INSERT INTO message_relations (parent_id, child_id) VALUES (?, ?)",
                        (prev_msg_id, current_msg_id)
                    )

                prev_msg_id = current_msg_id
                last_msg_id = current_msg_id

            # 更新对话最后消息 ID 和更新时间
            cursor.execute(
                "UPDATE conversations SET last_message_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                (last_msg_id, conversation_id)
            )

            conn.commit()
            print(f"成功向对话 {conversation_id} 追加了 {len(messages)} 条消息。")

        except Exception as e:
            conn.rollback()
            raise e

        finally:
            conn.close()
    
    @staticmethod
    def get_conversation_messages(conversation_id: int) -> str:
        """
        高效获取对话的完整消息链：从最后一条消息向上追溯所有父节点，构建顺序列表。
        一次性查询所有涉及的消息与关系，避免多次数据库调用。
        """
        conn = get_db_connection()
        cursor = conn.cursor()

        # 获取最后一条消息 ID
        cursor.execute(
            "SELECT last_message_id FROM conversations WHERE id = ?",
            (conversation_id,)
        )
        result = cursor.fetchone()
        if not result or not result["last_message_id"]:
            conn.close()
            print("该对话没有消息。")
            return "[]"
        
        last_msg_id = result["last_message_id"]

        # 获取当前对话中所有消息
        cursor.execute(
            "SELECT id, role, content FROM messages WHERE owned_conversation = ?",
            (conversation_id,)
        )
        all_messages = {row["id"]: {"role": row["role"], "content": row["content"]} for row in cursor.fetchall()}

        # 获取所有 message_relations 映射 child -> parent
        cursor.execute(
            "SELECT parent_id, child_id FROM message_relations "
            "WHERE child_id IN (SELECT id FROM messages WHERE owned_conversation = ?)",
            (conversation_id,)
        )
        child_to_parent = {row["child_id"]: row["parent_id"] for row in cursor.fetchall()}

        conn.close()

        # 从最后一条消息向上回溯链条
        message_chain = []
        current_id = last_msg_id
        while current_id:
            msg = all_messages.get(current_id)
            if msg:
                message_chain.append(msg)
            current_id = child_to_parent.get(current_id)

        message_chain.reverse()  # 转换为从头到尾顺序

        json_output = json.dumps(message_chain, ensure_ascii=False, indent=2)

        return json_output
