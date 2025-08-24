# neochat/game/events/system_action.py
from typing import Dict
from .base import BaseEventHandler

# 导入新架构下的模块
from neochat.llm.client import chat_with_deepseek # LLM客户端
from neochat.platform.logging import log_error, log_info_color, log_debug, TermColors # 日志和颜色
from neochat.memory.manager import MemoryManager # 记忆管理器
from neochat.platform.configuration import config # 配置对象

class SystemActionEventHandler(BaseEventHandler):
    """
    处理系统级别的AI动作，例如由LLM在后台生成内容并设置到游戏变量中。
    """
    def handle(self, params: Dict, content: str) -> bool:
        tool = params.get('Tool')
        var_name = params.get('Variable')
        if not tool or not var_name:
            log_error(f"SystemAction 事件缺少 'Tool' 或 'Variable' 参数: {params}")
            return True

        if tool == 'Generate':
            log_info_color("AI 正在幕后构思剧情...", TermColors.MAGENTA)

            # 判断是否需要包含历史记录作为LLM上下文
            include_history = str(params.get('IncludeHistory', 'false')).lower() == 'true'
            final_user_prompt = content # 默认用户提示即为事件内容

            messages = [{"role": "system", "content": "你是一个富有创造力的游戏剧本助手。请根据以下要求完成任务，并直接输出结果，不要包含任何额外解释。"}]

            if include_history:
                log_debug("SystemAction: 检测到 IncludeHistory=true，正在构建历史上下文...")
                memory_manager = MemoryManager(self.state)
                context_messages = memory_manager.get_context_for_llm(
                    history_limit=config.llm.conversation_history_limit,
                    perspective_char_id=None # 系统视角
                )
                # 将历史上下文作为单独的 system 消息块添加到 messages 中
                if context_messages:
                    history_text_block = "\n".join([msg['content'] for msg in context_messages if msg['role'] != 'system'])
                    messages.append({
                        "role": "system",
                        "content": (
                            "以下是到目前为止的对话历史记录，请将其作为背景参考。\n"
                            "--- 历史消息开始 ---\n"
                            f"{history_text_block}\n"
                            "--- 历史消息结束 ---\n\n"
                            "现在，请严格按照下面的指示完成你的任务："
                        )
                    })
                # 将原始的 content 作为用户提示，衔接在历史上下文之后
                messages.append({"role": "user", "content": content})
            else:
                # 如果不包含历史，则直接将 content 作为用户提示
                messages.append({"role": "user", "content": content})

            generated_content = chat_with_deepseek(messages, character_name="幕后导演", is_internal_thought=True)
            if generated_content:
                self.state.set_variable(var_name, generated_content.strip())
                log_debug(f"SystemAction 执行完毕, 变量 '{var_name}' 已设置。")
            else:
                log_error(f"SystemAction 未能从LLM生成内容，变量 '{var_name}' 未设置。")
        else:
            log_error(f"未知的 SystemAction Tool: {tool}")

        return True # SystemAction 通常不阻塞游戏流程