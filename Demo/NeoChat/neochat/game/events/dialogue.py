# neochat/game/events/dialogue.py
from typing import Dict
from .base import BaseEventHandler

# 导入新架构下的模块
from neochat.llm.client import chat_with_deepseek # LLM客户端
from neochat.platform.logging import log_error, TermColors # 日志和颜色
from neochat.memory.manager import MemoryManager # 记忆管理器
from neochat.platform.configuration import config # 配置对象

class DialogueEventHandler(BaseEventHandler):
    """
    处理对话事件，可以是预设文本或由LLM生成。
    """
    def handle(self, params: Dict, content: str) -> bool:
        # 格式化角色ID，支持变量替换
        char_id = self.state.format_string(params['Character'])
        character = self.state.session.characters.get(char_id)
        if not character:
            log_error(f"对话事件错误: 找不到角色ID '{char_id}'")
            return True # 找不到角色，事件结束，游戏继续

        if params.get('Mode') == 'Preset':
            # 预设对话模式，直接显示文本
            self.ui.display_dialogue(character.name, content)
            self.state.add_dialogue_history('Dialogue', character_id=char_id, content=content)

        elif params.get('Mode') == 'Prompt':
            # LLM生成对话模式
            # 1. 准备基础消息 (LLM的系统提示词，即角色人设)
            messages = [{"role": "system", "content": self.state.format_string(character.prompt)}]

            # 2. 使用 MemoryManager 获取完整的上下文 (包括RAG检索和最近历史)
            memory_manager = MemoryManager(self.state)
            context_messages = memory_manager.get_context_for_llm(
                history_limit=config.llm.conversation_history_limit, # 从配置中获取历史限制
                perspective_char_id=char_id # 以当前发言角色为视角
            )
            messages.extend(context_messages) # 将上下文添加到消息列表

            # 3. 添加当前事件的即时指令 (作为用户消息，引导LLM生成特定内容)
            if content and content.strip():
                final_prompt = (
                    f"System: 根据以上对话历史和你的记忆，请你做出回应。你的内心想法或行动指引是：\n"
                    f"{content}\n"
                    f"请直接生成你的对话，不要带上内心独白或额外解释。"
                )
                messages.append({"role": "user", "content": final_prompt})

            # 4. 调用LLM获取回应
            response = chat_with_deepseek(messages, character.name, color_code=TermColors.CYAN)
            if response:
                self.state.add_dialogue_history('Dialogue', character_id=char_id, content=response)
            else:
                log_error(f"LLM未能为角色 '{character.name}' 生成响应。")
        return True # LLM生成对话不阻塞游戏流程