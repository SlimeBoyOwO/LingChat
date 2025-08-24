# core/events/system_action.py
from typing import Dict
from .base import BaseEventHandler
from ..llm_interface import chat_with_deepseek
from ..logger import log_error, log_info_color, log_debug, TermColors
import config

class SystemActionEventHandler(BaseEventHandler):
    def handle(self, params: Dict, content: str) -> bool:
        tool = params.get('Tool')
        var_name = params.get('Variable')
        if not tool or not var_name:
            log_error(f"SystemAction 事件缺少 Tool 或 Variable 参数: {params}")
            return True

        if tool == 'Generate':
            log_info_color("AI 正在幕后构思剧情...", TermColors.MAGENTA)
            
            include_history = str(params.get('IncludeHistory', 'false')).lower() == 'true'
            final_user_prompt = content

            if include_history:
                log_debug("SystemAction: 检测到 IncludeHistory=true，正在构建历史上下文...")
                history_count = config.LLM_CONVERSATION_HISTORY_LIMIT 
                formatted_history_lines = []
                player_name = self.state.session.player.name or "玩家"

                for record in self.state.dialogue_history[-history_count:]:
                    record_content = record.get('content') or record.get('data', {}).get('content')
                    if not record_content: continue

                    line = ""
                    record_type = record.get('type')

                    if record_type == 'Dialogue':
                        char_id = record.get('data', {}).get('character_id')
                        char_name = self.state.session.characters.get(char_id, type('',(object,),{'name':'未知角色'})()).name
                        line = f"{char_name}: {record_content}"
                    elif record_type == 'Player':
                        line = f"{player_name}: {record_content}"
                    elif record_type == 'Narration':
                        line = f"旁白: {record_content}"
                    
                    if line: formatted_history_lines.append(line.strip())

                if formatted_history_lines:
                    history_text_block = "\n".join(formatted_history_lines)
                    final_user_prompt = (
                        "以下是到目前为止的对话历史记录，请将其作为背景参考。\n"
                        "--- 历史消息开始 ---\n"
                        f"{history_text_block}\n"
                        "--- 历史消息结束 ---\n\n"
                        "现在，请严格按照下面的指示完成你的任务：\n\n"
                        f"{content}"
                    )
                
            system_prompt = "你是一个富有创造力的游戏剧本助手。请根据以下要求完成任务，并直接输出结果，不要包含任何额外解释。"
            messages = [{"role": "system", "content": system_prompt}, {"role": "user", "content": final_user_prompt}]

            generated_content = chat_with_deepseek(messages, character_name="幕后导演", is_internal_thought=True)
            if generated_content:
                self.state.set_variable(var_name, generated_content.strip())
                log_debug(f"SystemAction 执行完毕, 变量 '{var_name}' 已设置。")
            else:
                log_error(f"SystemAction 未能从LLM生成内容，变量 '{var_name}' 未设置。")
        else:
            log_error(f"未知的 SystemAction Tool: {tool}")
            
        return True