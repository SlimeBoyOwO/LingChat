from typing import Optional, List, Dict, Any

from ling_chat.game_database.models import GameLine, LineBase

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
                    a = current_buffer[i].attribute
                    try:
                        a_val = a.value
                    except Exception:
                        a_val = str(a)
                    if a_val != 'user':
                        split_index = i + 1
                        break
                    if i == 0 and a_val == 'user':
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

        def _attr_value(line: GameLine) -> str:
            a = line.attribute
            try:
                return a.value
            except Exception:
                return str(a)

        for line in lines:
            line_obj = line
            
            # System 处理
            if _attr_value(line) == 'system':
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