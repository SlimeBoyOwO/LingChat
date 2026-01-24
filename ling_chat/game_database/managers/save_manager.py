from typing import List, Optional, Dict, Any
from datetime import datetime
from sqlmodel import Session, select
from ling_chat.game_database.database import engine
from ling_chat.game_database.models import LineBase, Save, Line, RunningScript

class SaveManager:
    @staticmethod
    def get_save_by_id(save_id: int) -> Optional[Save]:
        """通过ID获取存档（简单版）"""
        with Session(engine, expire_on_commit=False) as session:
            return session.get(Save, save_id)
        
    @staticmethod
    def create_save(user_id: int, title: str) -> Save:
        with Session(engine, expire_on_commit=False) as session:
            save = Save(title=title, user_id=user_id)
            session.add(save)
            session.commit()
            session.refresh(save)
            return save

    @staticmethod
    def get_user_saves(user_id: int, page: int = 1, page_size: int = 10) -> Dict[str, Any]:
        with Session(engine, expire_on_commit=False) as session:
            offset = (page - 1) * page_size
            
            # Total count
            count_stmt = select(Save).where(Save.user_id == user_id)
            # using len(all()) is easiest for now
            total = len(session.exec(count_stmt).all())
            
            # Paged results
            stmt = select(Save).where(Save.user_id == user_id).order_by(Save.update_date.desc()).offset(offset).limit(page_size)
            saves = session.exec(stmt).all()
            
            return {
                "saves": saves,
                "total": total
            }
        
    @staticmethod
    def update_save_main_role(save_id: int, role_id: Optional[int] = None) -> Save:
        """
        更新存档的主要角色ID
        
        Args:
            save_id: 存档ID
            role_id: 角色ID，为None时表示清除主要角色
            
        Returns:
            更新后的Save对象
            
        Raises:
            ValueError: 存档不存在
        """
        with Session(engine, expire_on_commit=False) as session:
            save = session.get(Save, save_id)
            if not save:
                raise ValueError("Save not found")
            
            save.main_role_id = role_id
            save.update_date = datetime.now()
            
            session.add(save)
            session.commit()
            session.refresh(save)
            
            return save
        
    @staticmethod
    def update_save_title(save_id: int, title: str) -> Save:
        """
        更新存档的主要角色ID
        
        Args:
            save_id: 存档ID
            title: 存档标题
            
        Returns:
            更新后的Save对象
            
        Raises:
            ValueError: 存档不存在
        """
        with Session(engine, expire_on_commit=False) as session:
            save = session.get(Save, save_id)
            if not save:
                raise ValueError("Save not found")
            
            save.title = title
            
            session.add(save)
            session.commit()
            session.refresh(save)
            
            return save 

    @staticmethod
    def append_message(save_id: int, 
                       content: str, 
                       attribute: str, 
                       role_id: Optional[int] = None,
                       display_name: Optional[str] = None,
                       audio_file: Optional[str] = None) -> Line:
        """
        向存档追加一条消息。自动处理 parent_line_id 和更新 Save.last_message_id
        """
        return SaveManager.append_messages(save_id, [{
            "content": content,
            "attribute": attribute,
            "role_id": role_id,
            "display_name": display_name,
            "audio_file": audio_file
        }])[0]

    @staticmethod
    def append_messages(save_id: int, messages: List[Dict[str, Any]]) -> List[Line]:
        """
        批量追加消息，支持完整的 Line 字段。
        messages: List[Dict] containing Line fields (content, attribute, role_id, etc.)
        """
        if not messages:
            return []
            
        created_lines = []
        
        with Session(engine, expire_on_commit=False) as session:
            save = session.get(Save, save_id)
            if not save:
                raise ValueError(f"Save {save_id} not found")
            
            current_parent_id = save.last_message_id
            
            for msg_data in messages:
                # 过滤出 Line 模型支持的字段
                line_data = {
                    "save_id": save_id,
                    "parent_line_id": current_parent_id
                }
                
                # 显式复制字段以确保安全和类型，或者直接 try key mapping
                valid_fields = [
                    "content", "attribute", "role_id", "script_role_id", "display_name",
                    "original_emotion", "predicted_emotion", "tts_content", "action_content", "audio_file"
                ]
                
                for field in valid_fields:
                    if field in msg_data:
                        line_data[field] = msg_data[field]
                
                # 创建 Line 对象
                new_line = Line.model_validate(line_data)
                session.add(new_line)
                session.commit()
                session.refresh(new_line)
                
                created_lines.append(new_line)
                current_parent_id = new_line.id
            
            # 更新存档指针
            if created_lines:
                save.last_message_id = created_lines[-1].id
                save.update_date = datetime.now()
                session.add(save)
                session.commit()
        
        return created_lines

    @staticmethod
    def get_line_list(save_id: int) -> List[Line]:
        """
        获取完整对话历史，从根节点到 last_message_id
        """
        with Session(engine, expire_on_commit=False) as session:
            save = session.get(Save, save_id)
            if not save or not save.last_message_id:
                return []
            
            # 获取该存档所有台词
            statement = select(Line).where(Line.save_id == save_id)
            lines = session.exec(statement).all()
            lines_map = {line.id: line for line in lines}
            
            history = []
            current_id = save.last_message_id
            
            while current_id:
                line = lines_map.get(current_id)
                if not line:
                    break
                history.append(line)
                current_id = line.parent_line_id
            
            return list(reversed(history))
    
    @staticmethod
    def get_linebase_list(save_id: int) -> List[LineBase]:
        """
        获取完整对话历史，从根节点到 last_message_id，但这里返回的是 LineBase
        """
        with Session(engine, expire_on_commit=False) as session:
            save = session.get(Save, save_id)
            if not save or not save.last_message_id:
                return []
            
            # 获取该存档所有台词
            statement = select(Line).where(Line.save_id == save_id)
            lines = session.exec(statement).all()
            lines_map = {line.id: line for line in lines}
            
            history = []
            current_id = save.last_message_id
            
            while current_id:
                line = lines_map.get(current_id)
                if not line:
                    break
                history.append(line)
                current_id = line.parent_line_id
            
            return list(reversed(history))
    
    @staticmethod
    def sync_lines(save_id: int, input_lines: List[LineBase]) -> bool:
        """
        智能同步台词列表 (v2.0 ID增强版)：
        1. 优先尝试 ID 匹配：如果 ID 相同但内容不同，直接 Update DB。
        2. ID 不符或内容不符时，视为分叉点 (Divergence)。
        3. 删除分叉点之后的所有旧台词。
        4. 追加分叉点之后的所有新台词。
        
        保留了历史数据的 ID 稳定性。
        """
        with Session(engine, expire_on_commit=False) as session:
            save = session.get(Save, save_id)
            if not save:
                raise ValueError(f"Save {save_id} not found")

            # 1. 获取现有 DB 台词 (构建有序链表)
            stmt_all = select(Line).where(Line.save_id == save_id)
            db_lines_raw = session.exec(stmt_all).all()
            lines_map = {line.id: line for line in db_lines_raw}
            
            db_history: List[Line] = []
            curr_id = save.last_message_id
            while curr_id:
                l = lines_map.get(curr_id)
                if not l: break
                db_history.append(l)
                curr_id = l.parent_line_id
            db_history.reverse() # [Line1, Line2, Line3...]

            # 2. 寻找分叉点 & 执行更新
            divergence_index = 0
            limit = min(len(db_history), len(input_lines))
            
            # 需要检查更新的字段
            update_fields = [
                "content", "attribute", "role_id", "script_role_id", "display_name",
                "original_emotion", "predicted_emotion", "tts_content", 
                "action_content", "audio_file"
            ]
            
            while divergence_index < limit:
                db_line = db_history[divergence_index]
                in_line = input_lines[divergence_index]
                
                is_match = False
                
                # [情况 A] 输入行有 ID -> 强校验
                if in_line.id is not None:
                    if in_line.id == db_line.id:
                        is_match = True
                        # 检查是否需要 Update
                        need_update = False
                        for field in update_fields:
                            # 获取新值
                            new_val = getattr(in_line, field)
                            # 获取旧值
                            old_val = getattr(db_line, field)
                            
                            if new_val != old_val:
                                setattr(db_line, field, new_val)
                                need_update = True
                        
                        if need_update:
                            session.add(db_line) # 标记为更新
                    else:
                        # ID 不同，说明链条断裂或发生了插入/删除
                        is_match = False
                
                # [情况 B] 输入行无 ID -> 弱校验 (内容比对)
                else:
                    # 如果内容等核心字段一致，我们认为是同一行，只是输入对象丢失了ID
                    # 这里简化判定：只比较 content 和 attribute
                    if (db_line.content == in_line.content and 
                        db_line.attribute == in_line.attribute and
                        db_line.role_id == in_line.role_id):
                        is_match = True
                        # 可以在这里做一些非核心字段的 update，略
                    else:
                        is_match = False

                if not is_match:
                    break # 发现分叉，停止比对
                
                divergence_index += 1

            # 3. 处理删除 (Prune)
            # 任何在 divergence_index 之后的 DB 数据都是“旧分支”，需要切除
            if divergence_index < len(db_history):
                lines_to_delete = db_history[divergence_index:]
                for l in lines_to_delete:
                    session.delete(l)
                
                # 修正指针：回退到分叉点的前一个
                if divergence_index > 0:
                    current_parent_id = db_history[divergence_index - 1].id
                else:
                    current_parent_id = None # 删光了
            else:
                # 没发生删除，父节点就是 DB 的最后一个
                if db_history:
                    current_parent_id = db_history[-1].id
                else:
                    current_parent_id = None

            # 4. 处理追加 (Append)
            lines_to_add = input_lines[divergence_index:]
            created_lines = []

            if lines_to_add:
                for line_base in lines_to_add:
                    # 转换 LineBase -> Line
                    # 关键：一定要 exclude={'id'}，防止如果 input_line 自带了一个错误的 ID 导致主键冲突
                    # 追加的新行，必须由 DB 生成新 ID
                    line_data = line_base.model_dump(exclude={"id"}) 
                    
                    line_data["save_id"] = save_id
                    line_data["parent_line_id"] = current_parent_id
                    
                    new_line = Line.model_validate(line_data)
                    session.add(new_line)
                    session.commit()
                    session.refresh(new_line)
                    
                    current_parent_id = new_line.id
                    created_lines.append(new_line)
            
            # 5. 最终更新 Save 指针
            # 最后的 ID 可能是新追加的最后一个，或者是没被删掉的旧历史的最后一个
            final_last_id = None
            if created_lines:
                final_last_id = created_lines[-1].id
            elif divergence_index > 0:
                final_last_id = db_history[divergence_index - 1].id
            
            save.last_message_id = final_last_id
            save.update_date = datetime.now()
            session.add(save)
            
            session.commit()
            return True
    
    @staticmethod
    def get_chat_main_character_id(save_id: int) -> int | None:
        """
        获取主角
        """
        return SaveManager.get_line_list(save_id)[0].role_id

    @staticmethod
    def update_running_script(save_id: int, script_data: Dict[str, Any]):
        """
        更新或创建运行剧本状态
        script_data 包含: script_folder, variable_info, current_chapter, event_sequence
        """
        with Session(engine, expire_on_commit=False) as session:
            save = session.get(Save, save_id)
            if not save:
                raise ValueError("Save not found")
            
            if save.running_script_id:
                # Update existing
                script = session.get(RunningScript, save.running_script_id)
                if script:
                    for key, value in script_data.items():
                        if hasattr(script, key):
                            setattr(script, key, value)
                    session.add(script)
            else:
                # Create new
                script = RunningScript(save_id=save_id, **script_data)
                session.add(script)
                session.commit()
                session.refresh(script)
                
                save.running_script_id = script.id
                session.add(save)
            
            session.commit()

    @staticmethod
    def delete_save(save_id: int) -> bool:
        with Session(engine, expire_on_commit=False) as session:
            save = session.get(Save, save_id)
            if save:
                session.delete(save)
                session.commit()
                return True
            return False
