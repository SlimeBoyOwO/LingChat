from typing import Optional, List, Dict, Any, Set

from ling_chat.game_database.models import LineBase
from ling_chat.core.ai_service.type import GameRole

class MemoryBuilder:
    def __init__(self, target_role_id: Optional[int] = None, 
                 target_script_role_id: Optional[str] = None, 
                 target_display_name: Optional[str] = None):
        """
        初始化构建器，指定当前需要构建记忆的主角(AI)身份。
        """
        self.target_role_id = target_role_id
        self.target_script_role_id = target_script_role_id
        self.target_display_name = target_display_name

    def _is_target(self, line: LineBase) -> bool:
        """判断某一行是否属于当前目标AI角色"""
        if self.target_role_id is not None and line.role_id == self.target_role_id:
            return True
        if self.target_script_role_id is not None and line.script_role_id == self.target_script_role_id:
            return True
        if self.target_display_name is not None and line.display_name == self.target_display_name:
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
        适用于 User Block 中的非 User 消息，或者 Context 中的 User 消息
        """
        # 注意：User一般没有 emotion/tts/action，但如果有（比如NPC），也会被格式化出来
        # User 在背景中也需要显示 DisplayName
        content_str = self._format_content_with_extras(line)
        name = line.display_name if line.display_name else "用户"
        return f"{name}: {content_str}"

    def build(self, lines: List[LineBase]) -> List[Dict[str, Any]]:
        """
        核心构建函数
        """
        memory = []
        
        # 缓冲区，用于存储连续的同类型消息以便合并
        # buffer_type: 'target_assistant' 或 'other_block' (包含 user 和 非 target assistant)
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
                # 逻辑：找到最后一段连续的 User 消息，放在大括号外；前面的全部放在大括号内
                
                # 1. 寻找分割点
                split_index = len(current_buffer)
                # 从后往前找，直到找到非 User 的行 (即非 Target 的 Assistant/NPC)
                for i in range(len(current_buffer) - 1, -1, -1):
                    if current_buffer[i].attribute != 'user':
                        # 找到 NPC 发言，分割点就在它后面
                        split_index = i + 1
                        break
                    # 如果一直是 user，split_index 会一直变小直到 0
                    if i == 0 and current_buffer[i].attribute == 'user':
                        split_index = 0

                context_lines = current_buffer[:split_index]
                active_user_lines = current_buffer[split_index:]

                final_content_parts = []

                # 2. 构建大括号内的 Context 部分
                if context_lines:
                    context_strs = [self._format_context_line(l) for l in context_lines]
                    # 用换行符连接背景对话
                    joined_context = "\n".join(context_strs)
                    final_content_parts.append(f"{{{joined_context}}}")

                # 3. 构建大括号外的 Active User 部分
                if active_user_lines:
                    # User 连续发言直接拼接 (参考示例 1 和 2 的处理)
                    # 示例 2 中: "真是的..." 和 "钦灵酱..." 看起来是直接拼接或空格，但示例Json中有 \n
                    # 根据 User 提示： "last_user_content current_user_content" 
                    # 观察示例 2 的 Result Memory，Msg 6 User 部分：
                    # content: "{...}\n真是的...钦灵酱，换好了吗？"
                    # 这里使用了直接拼接 (string concatenation)。
                    
                    user_text = "".join([l.content for l in active_user_lines])
                    final_content_parts.append(user_text)

                # 4. 组合最终字符串
                # 如果有 context 也有 user，中间一般加个换行符分隔 (参考示例2 Msg6)
                if context_lines and active_user_lines:
                    final_content_str = "\n".join(final_content_parts)
                else:
                    final_content_str = "".join(final_content_parts)

                memory.append({
                    "role": "user",
                    "content": final_content_str
                })

            # 清空缓冲
            current_buffer = []
            buffer_type = None

        # 主循环
        for line in lines:
            # 1. 处理 System (不参与缓冲，直接插入，除非要保序)
            # 根据逻辑，System 是独立的。如果在中间出现，通常应该打断当前的缓冲吗？
            # 假设 System 独立处理，不影响 User/Assistant 的连续性判定比较复杂。
            # 这里简单起见：System 出现会触发 Flush，保持时间线顺序。
            if line.attribute == 'system':
                if self._is_target(line):
                    flush_buffer()
                    memory.append({
                        "role": "system",
                        "content": line.content
                    })
                continue # 如果不是 target 的 system，直接忽略

            # 2. 判断当前行类型
            is_target_line = (line.attribute == 'assistant' and self._is_target(line))
            
            if is_target_line:
                # 如果当前是 Target Assistant
                if buffer_type == 'other_block':
                    flush_buffer()
                
                buffer_type = 'target_assistant'
                current_buffer.append(line)
            
            else:
                # 如果是 User 或 Non-Target Assistant (NPC/旁白)
                if buffer_type == 'target_assistant':
                    flush_buffer()
                
                buffer_type = 'other_block'
                current_buffer.append(line)

        # 循环结束，刷新剩余缓冲
        flush_buffer()

        return memory

class RoleManager:
    """
    记忆管理器：负责记忆的增删改查、生命周期管理
    """
    def __init__(self):
        # 核心存储容器
        self._storage: Dict[str, GameRole] = {}

    def _get_key(self, role_id: Optional[int], script_role_id: Optional[str]) -> str:
        """生成唯一存储键"""
        if role_id is not None:
            return f"role_{role_id}"
        if script_role_id is not None:
            return f"script_{script_role_id}"
        raise ValueError("Role ID or Script Role ID must be provided")

    def _get_instance(self, role_id: Optional[int], script_role_id: Optional[str]) -> GameRole:
        """内部方法：获取或创建实例"""
        key = self._get_key(role_id, script_role_id)
        if key not in self._storage:
            self._storage[key] = GameRole(role_id=role_id, script_role_id=script_role_id)
        return self._storage[key]

    # -----------------------
    # 公开 API
    # -----------------------

    def get_history(self, role_id: Optional[int] = None, script_role_id: Optional[str] = None) -> List[Dict]:
        """获取某角色的所有对话历史"""
        instance = self._get_instance(role_id, script_role_id)
        return instance.memory

    def get_role(self, role_id: Optional[int] = None, script_role_id: Optional[str] = None) -> GameRole:
        """获取某角色的实例"""
        return self._get_instance(role_id, script_role_id)

    def exists(self, role_id: Optional[int] = None, script_role_id: Optional[str] = None) -> bool:
        """判断是否已有记忆"""
        try:
            key = self._get_key(role_id, script_role_id)
            return key in self._storage
        except ValueError:
            return False

    def _update_role_metadata(self, instance: GameRole, lines: List[LineBase]):
        """
        根据传入的台词列表，更新 GameRole 实例的属性（如 display_name）。
        逻辑：倒序遍历寻找属于该角色的最后一句台词。
        """
        target_line: Optional[LineBase] = None

        # 1. 倒序遍历：找到该角色说的最后一句台词
        # 这里的 lines 通常已经是 source_lines（如最近 50 条）
        for line in reversed(lines):
            # 检查 role_id 匹配
            if instance.role_id is not None and line.role_id == instance.role_id:
                target_line = line
                break
            # 检查 script_role_id 匹配
            if instance.script_role_id is not None and line.script_role_id == instance.script_role_id:
                target_line = line
                break
        
        # 2. 如果找到了台词，进行属性同步
        if target_line:
            # Update: display_name
            # 如果台词中有 display_name，则覆盖；这允许名字随剧情变化（例如 "神秘人" -> "阿良"）
            if target_line.display_name:
                instance.display_name = target_line.display_name
            
            # [扩展点 A]：在这里可以更新其他从 Line 中获取的动态属性
            # 例如：instance.current_emotion = target_line.predicted_emotion

        # [扩展点 B]：如果 instance 是新创建的，且某些属性为空，
        # 可以在这里触发数据库查询来填充静态属性（如 resource_path, prompt）
        # if not instance.resource_path and instance.role_id:
        #     role_db = db_session.get(Role, instance.role_id)
        #     if role_db:
        #         instance.resource_path = role_db.resource_folder

    def refresh_memories_from_lines(self, lines: List[LineBase], recent_n: Optional[int] = None):
        """
        根据传入的台词列表，自动识别所有角色并构建记忆。
        同时会同步角色的最新状态（如显示名称）。
        """
        # 1. 确定数据源
        source_lines = lines[-recent_n:] if recent_n else lines

        # 2. 扫描当前上下文中“活跃”的 ID
        active_role_ids: Set[int] = {l.role_id for l in source_lines if l.role_id is not None}
        active_script_ids: Set[str] = {l.script_role_id for l in source_lines if l.script_role_id is not None}

        active_keys: Set[str] = set()

        # 3. 为每个 role_id 处理
        for rid in active_role_ids:
            # 3.1 获取或创建实例
            instance = self._get_instance(role_id=rid, script_role_id=None)
            
            # 3.2 [升级] 同步元数据 (display_name 等)
            self._update_role_metadata(instance, source_lines)
            
            # 3.3 构建并更新记忆
            builder = MemoryBuilder(target_role_id=rid)
            memory = builder.build(source_lines)
            # 直接赋值，省去一次 _get_instance 调用
            instance.memory = memory
            
            active_keys.add(self._get_key(role_id=rid, script_role_id=None))

        # 4. 为每个 script_role_id 处理
        for sid in active_script_ids:
            # 4.1 获取或创建实例
            instance = self._get_instance(role_id=None, script_role_id=sid)
            
            # 4.2 [升级] 同步元数据
            self._update_role_metadata(instance, source_lines)
            
            # 4.3 构建并更新记忆
            builder = MemoryBuilder(target_script_role_id=sid)
            memory = builder.build(source_lines)
            instance.memory = memory
            
            active_keys.add(self._get_key(role_id=None, script_role_id=sid))

        # 5. 垃圾回收
        existing_keys = set(self._storage.keys())
        stale_keys = existing_keys - active_keys

        # TODO: 这里的垃圾回收似乎有问题，这个Gemini写的寄吧函数太他妈冗余了，之后可以考虑重构一下
        
        for key in stale_keys:
            del self._storage[key]