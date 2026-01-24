from typing import Optional, List, Dict, Any, Set
from sqlmodel import Session, select

from ling_chat.game_database.models import GameLine, Line, Role, RoleType, LineBase
from ling_chat.core.ai_service.type import GameRole
from ling_chat.game_database.database import engine

class MemoryBuilder:
    def __init__(self, target_role_id: int, target_display_name: Optional[str] = None):
        """
        初始化构建器，指定当前需要构建记忆的主角(AI)身份。
        """
        self.target_role_id = target_role_id
        self.target_display_name = target_display_name

    def _is_target(self, line: GameLine) -> bool:
        """
        判断某一行是否被当前目标AI角色“感知”。
        感知规则：
        1. 是自己说的 (sender_role_id == target_role_id)
        2. 或者是自己听到的 (在 perceived 列表中)
        """
        # 1. 自己说的
        if line.sender_role_id == self.target_role_id:
            return True
            
        # 2. 被感知的
        for role_id in line.perceived_role_ids:
            if role_id == self.target_role_id:
                return True
        
        return False

    def _format_content_with_extras(self, line: LineBase) -> str:
        """
        格式化内容：【情绪】内容<TTS>（动作）
        适用于 Assistant 消息
        """
        parts = []
        
        # 情绪
        if line.original_emotion:
            parts.append(f"【{line.original_emotion}】")
        
        # 正文
        parts.append(line.content)
        
        # TTS
        if line.tts_content:
            parts.append(f"<{line.tts_content}>")
        
        # 动作
        if line.action_content:
            parts.append(f"（{line.action_content}）")
            
        return "".join(parts)

    def _format_context_line(self, line: LineBase) -> str:
        """
        格式化大括号内的背景行：Display: 【情绪】内容<TTS>（动作）
        """
        content_str = self._format_content_with_extras(line)
        name = line.display_name if line.display_name else "未知"
        return f"{name}: {content_str}"

    def build(self, lines: List[GameLine]) -> List[Dict[str, Any]]:
        """
        核心构建函数
        """
        memory = []
        
        current_buffer: List[LineBase] = []
        buffer_type: Optional[str] = None

        def flush_buffer():
            nonlocal current_buffer, buffer_type
            if not current_buffer:
                return

            if buffer_type == 'target_assistant':
                # 合并 Target Assistant 消息
                full_content = "".join([self._format_content_with_extras(l) for l in current_buffer])
                memory.append({
                    "role": "assistant",
                    "content": full_content
                })

            elif buffer_type == 'other_block':
                # 处理 User + Context 混合块
                split_index = len(current_buffer)
                for i in range(len(current_buffer) - 1, -1, -1):
                    if current_buffer[i].attribute != 'user':
                        split_index = i + 1
                        break
                    if i == 0 and current_buffer[i].attribute == 'user':
                        split_index = 0

                context_lines = current_buffer[:split_index]
                active_user_lines = current_buffer[split_index:]

                final_content_parts = []

                if context_lines:
                    context_strs = [self._format_context_line(l) for l in context_lines]
                    joined_context = "\n".join(context_strs)
                    final_content_parts.append(f"{{{joined_context}}}")

                if active_user_lines:
                    user_text = "".join([l.content for l in active_user_lines])
                    final_content_parts.append(user_text)

                if context_lines and active_user_lines:
                    final_content_str = "\n".join(final_content_parts)
                else:
                    final_content_str = "".join(final_content_parts)

                memory.append({
                    "role": "user",
                    "content": final_content_str
                })

            current_buffer = []
            buffer_type = None

        for line in lines:
            line_obj = line # Type hinting alias
            
            # System 处理
            if line.attribute == 'system':
                # 系统消息只有感知到了才添加
                if self._is_target(line_obj):
                    flush_buffer()
                    memory.append({
                        "role": "system",
                        "content": line.content
                    })
                continue

            # 判断当前行类型
            # 注意：这里 _is_target 包含了 "是我说的" 或 "我听到了"
            is_perceived = self._is_target(line_obj)
            
            if not is_perceived:
                continue # 既没说也没听到，直接忽略，仿佛不存在

            # 如果是自己说的 (Assistant Role)
            is_self_speaking = (line.sender_role_id == self.target_role_id)
            
            if is_self_speaking:
                if buffer_type == 'other_block':
                    flush_buffer()
                
                buffer_type = 'target_assistant'
                current_buffer.append(line)
            
            else:
                # 别人说的 (User, NPC, Narrator) -> 归类为 Context (User Role block)
                if buffer_type == 'target_assistant':
                    flush_buffer()
                
                buffer_type = 'other_block'
                current_buffer.append(line)

        flush_buffer()
        return memory

class RoleManager:
    """
    记忆管理器：负责记忆的增删改查、生命周期管理
    """
    def __init__(self):
        # 核心存储容器: key = role_id (int)
        # 不再使用 complex keys, role_id is unique enough
        self._storage: Dict[int, GameRole] = {}

    def _get_instance(self, role_id: int) -> GameRole:
        """内部方法：获取或创建实例"""
        if role_id not in self._storage:
            self._storage[role_id] = GameRole(role_id=role_id)
        return self._storage[role_id]
    
    def _ensure_role_by_key(self, script_key: str) -> Optional[int]:
        """
        根据 script_key 查找 role_id。如果不存在，自动创建 NPC 角色。
        """
        with Session(engine) as session:
            statement = select(Role).where(Role.script_key == script_key)
            role = session.exec(statement).first()
            
            if not role:
                # 自动创建
                role = Role(
                    script_key=script_key,
                    name=script_key, # 默认名字
                    role_type=RoleType.NPC
                )
                session.add(role)
                session.commit()
                session.refresh(role)
            
            return role.id

    # -----------------------
    # 公开 API
    # -----------------------

    def get_role_by_script_key(self, script_key: str) -> GameRole:
        """通过剧本 Key 获取角色 (自动创建)"""
        role_id = self._ensure_role_by_key(script_key)
        if role_id:
             return self._get_instance(role_id)
        raise ValueError(f"Could not create role for key {script_key}")

    def get_history(self, role_id: int) -> List[Dict]:
        """获取某角色的所有对话历史"""
        instance = self._get_instance(role_id)
        return instance.memory

    def get_role(self, role_id: int) -> GameRole:
        """获取某角色的实例"""
        return self._get_instance(role_id)

    def exists(self, role_id: int) -> bool:
        """判断是否已有记忆"""
        return role_id in self._storage

    def _update_role_metadata(self, instance: GameRole, lines: List[GameLine]):
        """
        根据传入的台词列表，更新 GameRole 实例的属性（如 display_name）。
        """
        target_line: Optional[LineBase] = None

        # 1. 倒序遍历：找到该角色说的最后一句台词
        for line in reversed(lines):
            if line.sender_role_id == instance.role_id:
                target_line = line
                break
        
        # 2. 如果找到了台词，进行属性同步
        if target_line:
            if target_line.display_name:
                instance.display_name = target_line.display_name

    def refresh_memories_from_lines(self, lines: List[GameLine], recent_n: Optional[int] = None):
        """
        根据传入的台词列表，自动识别所有角色并构建记忆。
        """
        # 1. 确定数据源
        source_lines = lines[-recent_n:] if recent_n else lines

        # 2. 扫描当前上下文中“活跃”的 Role ID
        # 活跃 = 说过话的 OR 被感知到的
        active_role_ids: Set[int] = set()
        
        for l in source_lines:
            # Sender
            if l.sender_role_id:
                active_role_ids.add(l.sender_role_id)
            
            # Perceivers
            # 注意：l.perceived_by 必须被 eager loaded 或者 session active
            for p in l.perceived_role_ids:
                active_role_ids.add(p)
                        
        # 3. 为每个 role_id 处理
        for rid in active_role_ids:
            # 3.1 获取或创建实例
            instance = self._get_instance(role_id=rid)
            
            # 3.2 同步元数据
            self._update_role_metadata(instance, source_lines)
            
            # 3.3 构建并更新记忆
            builder = MemoryBuilder(target_role_id=rid)
            memory = builder.build(source_lines)
            
            instance.memory = memory
            
        # 4. 垃圾回收 (Cleanup stale roles)
        existing_keys = set(self._storage.keys())
        stale_keys = existing_keys - active_role_ids
        
        for key in stale_keys:
            del self._storage[key]