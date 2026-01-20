import json
from typing import Dict, List, Optional

from ling_chat.database.database import Role, get_db_connection


class ConversationModel:
    @staticmethod
    def create_conversation(user_id: int, messages: List[Dict[str, str]], character_id: int, title: Optional[str] = None) -> int:
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
        # 优化设置（仅用于批量导入）
        conn.execute("PRAGMA synchronous = OFF")
        conn.execute("PRAGMA journal_mode = MEMORY")
        conn.execute("PRAGMA cache_size = 100000")

        cursor = conn.cursor()

        try:
            # 1. 插入对话
            cursor.execute(
                "INSERT INTO conversations (title, owned_user, character) VALUES (?, ?, ?)",
                (title, user_id, character_id)
            )
            conversation_id = cursor.lastrowid

            # 2. 批量插入消息（分批次防止SQL过长）
            BATCH_SIZE = 500  # 每次插入500条
            msg_ids = []

            for i in range(0, len(messages), BATCH_SIZE):
                batch = messages[i:i+BATCH_SIZE]
                placeholders = ",".join(["(?,?,?)"]*len(batch))
                params = []
                for msg in batch:
                    params.extend([msg["role"], msg["content"], conversation_id])

                cursor.execute(
                    f"INSERT INTO messages (role, content, owned_conversation) VALUES {placeholders}",
                    params
                )

                # 获取这批消息的ID（假设是连续分配的）
                first_id = cursor.lastrowid - len(batch) + 1
                msg_ids.extend(range(first_id, first_id + len(batch)))

            # 3. 批量插入关系（同样分批次）
            relations = [(msg_ids[i], msg_ids[i+1]) for i in range(len(msg_ids)-1)]
            for i in range(0, len(relations), BATCH_SIZE):
                batch = relations[i:i+BATCH_SIZE]
                placeholders = ",".join(["(?,?)"]*len(batch))
                params = [item for pair in batch for item in pair]

                cursor.execute(
                    f"INSERT INTO message_relations (parent_id, child_id) VALUES {placeholders}",
                    params
                )

            # 4. 更新最后消息
            cursor.execute(
                "UPDATE conversations SET last_message_id=?, updated_at=datetime('now') WHERE id=?",
                (msg_ids[-1], conversation_id)
            )

            conn.commit()
            return conversation_id

        except Exception as e:
            conn.rollback()
            raise e
        finally:
            # 恢复默认设置
            conn.execute("PRAGMA synchronous = NORMAL")
            conn.execute("PRAGMA journal_mode = DELETE")
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
    def change_conversation_messages(conversation_id: int, messages: List[Dict[str, str]]) -> None:
        """
        完全替换对话中的消息（删除原有消息，插入新消息）
        :param conversation_id: 要修改的对话ID
        :param messages: 新的消息列表，每条为{"role": "user/assistant/system", "content": "..."}
        """
        if not messages:
            raise ValueError("消息列表不能为空")

        conn = get_db_connection()
        # 优化设置（仅用于批量操作）
        conn.execute("PRAGMA synchronous = OFF")
        conn.execute("PRAGMA journal_mode = MEMORY")
        conn.execute("PRAGMA cache_size = 100000")

        cursor = conn.cursor()

        try:
            # 1. 验证对话存在性
            cursor.execute("SELECT id FROM conversations WHERE id = ?", (conversation_id,))
            if not cursor.fetchone():
                raise ValueError(f"对话 ID {conversation_id} 不存在")

            # 2. 删除原有消息和关系（外键约束应该会自动删除关系）
            cursor.execute("DELETE FROM messages WHERE owned_conversation = ?", (conversation_id,))

            # 3. 批量插入新消息（分批次防止SQL过长）
            BATCH_SIZE = 500  # 每次插入500条
            msg_ids = []

            for i in range(0, len(messages), BATCH_SIZE):
                batch = messages[i:i+BATCH_SIZE]
                placeholders = ",".join(["(?,?,?)"]*len(batch))
                params = []
                for msg in batch:
                    params.extend([msg["role"], msg["content"], conversation_id])

                cursor.execute(
                    f"INSERT INTO messages (role, content, owned_conversation) VALUES {placeholders}",
                    params
                )

                # 获取这批消息的ID（假设是连续分配的）
                first_id = cursor.lastrowid - len(batch) + 1
                msg_ids.extend(range(first_id, first_id + len(batch)))

            # 4. 批量插入新关系
            relations = [(msg_ids[i], msg_ids[i+1]) for i in range(len(msg_ids)-1)]
            for i in range(0, len(relations), BATCH_SIZE):
                batch = relations[i:i+BATCH_SIZE]
                placeholders = ",".join(["(?,?)"]*len(batch))
                params = [item for pair in batch for item in pair]

                cursor.execute(
                    f"INSERT INTO message_relations (parent_id, child_id) VALUES {placeholders}",
                    params
                )

            # 5. 更新最后消息和修改时间
            cursor.execute(
                "UPDATE conversations SET last_message_id = ?, updated_at = datetime('now') WHERE id = ?",
                (msg_ids[-1], conversation_id)
            )

            conn.commit()
            print(f"成功替换对话 {conversation_id} 的消息，共 {len(messages)} 条")

        except Exception as e:
            conn.rollback()
            raise e
        finally:
            # 恢复默认设置
            conn.execute("PRAGMA synchronous = NORMAL")
            conn.execute("PRAGMA journal_mode = DELETE")
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

    @staticmethod
    def update_conversation_title(conversation_id: int, title: str) -> None:
        """更新对话标题"""
        conn = get_db_connection()
        try:
            conn.execute(
                "UPDATE conversations SET title = ?, updated_at = datetime('now') WHERE id = ?",
                (title, conversation_id)
            )
            conn.commit()
        except Exception as e:
            conn.rollback()
            raise e
        finally:
            conn.close()

    @staticmethod
    def get_conversation_character(conversation_id: int) -> int:
        conn = get_db_connection()
        try:
            cursor = conn.execute(
                "SELECT character from conversations where id = ?",
                (conversation_id,)
            )
            result = cursor.fetchone()
            if result is None:
                return None
            return result[0]
        finally:
            conn.close()

    @staticmethod
    def delete_conversation(conversation_id: int) -> bool:
        """
        删除对话及其所有关联消息
        :param conversation_id: 要删除的对话ID
        :return: 是否删除成功
        """
        conn = get_db_connection()
        cursor = conn.cursor()
        cursor.execute("PRAGMA foreign_keys = ON")  # 必须对每个连接都开启

        try:
            # 由于外键约束，删除对话会自动删除关联的消息和关系
            cursor.execute(
                "DELETE FROM conversations WHERE id = ?",
                (conversation_id,)
            )

            # 检查是否真的删除了记录
            deleted = cursor.rowcount > 0

            conn.commit()
            return deleted

        except Exception as e:
            conn.rollback()
            raise e
        finally:
            conn.close()
