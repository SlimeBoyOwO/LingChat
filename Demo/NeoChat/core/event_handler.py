from typing import Any, Dict

from .state_manager import StateManager
from .ui import ConsoleUI
from .llm_interface import chat_with_deepseek
from .logger import log_debug, log_error, log_info, log_info_color, TermColors

import config

class EventHandler:
    """
    处理单个剧情事件，执行其逻辑。
    这是一个无状态的类，所有状态都由 StateManager 管理。
    """
    def __init__(self, state_manager: StateManager, ui: ConsoleUI):
        self.state = state_manager
        self.ui = ui
        self.handlers = {
            'Narration': self._handle_narration,
            'Dialogue': self._handle_dialogue,
            'Player': self._handle_player,
            'Action': self._handle_action,
            'Chapter': self._handle_chapter,
            'Notice': self._handle_notice,
            'PlayerNotice': self._handle_player_notice,
            'SystemAction': self._handle_system_action,
        }

    def handle_event(self, event_data: Dict[str, Any]) -> bool:
        """
        事件处理的入口点。
        返回 True 表示游戏可以继续自动执行，False 表示需要等待（如玩家输入）。
        """
        log_debug(f"处理事件: {event_data}")

        if 'Condition' in event_data:
            if not self.state.evaluate_condition(event_data['Condition']):
                log_debug("条件不满足，跳过事件块。")
                return True
            for nested_event in event_data.get('Events', []):
                if not self.handle_event(nested_event):
                    return False
            return True

        if not event_data:
            log_error("接收到空的事件数据。")
            return True

        event_key, event_content = list(event_data.items())[0]
        params = dict(param.strip().split(': ') for param in event_key.split(' | '))
        event_type = params.get('Type')

        if isinstance(event_content, str):
            content = self.state.format_string(event_content)
        elif isinstance(event_content, dict):
            content = {k: self.state.format_string(v) for k, v in event_content.items()}
        else:
            content = event_content

        handler = self.handlers.get(event_type)
        if handler:
            return handler(params, content)
        else:
            log_error(f"未知的事件类型: {event_type}")
            return True

    def _handle_narration(self, params: Dict, content: str) -> bool:
        if params.get('Mode') == 'Prompt':
            narrator_prompt = "你是一个优秀的、沉浸式的故事讲述者（旁白）。请根据以下要求和对话历史，生成一段富有文采的旁白。直接输出旁白内容，不要包含任何额外解释。"
            messages = [
                {"role": "system", "content": narrator_prompt},
                {"role": "user", "content": f"这是你的生成要求：\n{content}"}
            ]
            generated_content = chat_with_deepseek(messages, character_name="旁白", color_code=TermColors.GREY)
            if generated_content:
                self.state.add_dialogue_history('Narration', content=generated_content)
            else:
                log_error("旁白生成失败。")
        else:
            self.ui.display_narration(content)
            self.state.add_dialogue_history('Narration', content=content)
        return True

    def _handle_dialogue(self, params: Dict, content: str) -> bool:
        char_id = self.state.format_string(params['Character'])
        character = self.state.session.characters.get(char_id)
        if not character:
            log_error(f"对话事件错误: 找不到角色ID '{char_id}'")
            return True

        if params.get('Mode') == 'Preset':
            self.ui.display_dialogue(character.name, content)
            self.state.add_dialogue_history('Dialogue', character_id=char_id, content=content)
        
        elif params.get('Mode') == 'Prompt':
            messages = [{"role": "system", "content": self.state.format_string(character.prompt)}]
            # ※※※※※※※※※此处可以添加逻辑来构建更丰富的历史上下文※※※※※※※※※※
            messages.append({"role": "user", "content": f"System: 请据此做出回复，但是不要说出内心想法。以下是你的内心想法：\n {content}"})
            
            response = chat_with_deepseek(messages, character.name, color_code=TermColors.CYAN)
            if response:
                self.state.add_dialogue_history('Dialogue', character_id=char_id, content=response)
            else:
                log_error("LLM未能生成响应。")
        return True

    def _handle_player(self, params: Dict, content: str) -> bool:
        if params.get('Mode') == 'Input':
            self.state.progress.runtime_state = 'WaitingForPlayerInput'
            self.state.progress.context['prompt'] = content
            return False
        
        elif params.get('Mode') == 'Preset':
            self.ui.display_player_dialogue(self.state.session.player.name, content)
            self.state.add_dialogue_history('Player', content=content)
            return True
        return True

    def _handle_action(self, params: Dict, content: Dict) -> bool:
       tool = params.get('Tool')
       var_name = params.get('Variable')
    
       if tool == 'Set':
          self.state.set_variable(var_name, content.get('Value'))
       elif tool == 'Calculate':
          self.state.calculate_variable(var_name, content.get('Expression'))
       elif tool == 'Random':
          self.state.set_random_variable(var_name, params.get('Min'), params.get('Max'))
       elif tool == 'RandomChoice':
          self.state.set_random_choice_variable(var_name, content.get('Choices'))
       else:
          log_error(f"未知的 Action Tool: {tool}")
       return True

    def _handle_chapter(self, params: Dict, content: Dict) -> bool:
        self.ui.display_chapter(content.get('Title'), content.get('Description'))
        self.state.add_dialogue_history('Chapter', **content)
        return True

    def _handle_notice(self, params: Dict, content: str) -> bool:
        """处理游戏内公告，对玩家和LLM都可见。"""
        final_content = content
        if params.get('Mode') == 'Prompt':
            log_info("正在生成公告内容...")
            dm_char_id = self.state.session.story_pack_config.dm_role_id
            dm_char = self.state.session.characters.get(dm_char_id)
            
            dm_prompt = "你是一个剧本杀的DM（主持人）。"
            dm_name = "DM"
            if dm_char:
                dm_prompt = self.state.format_string(dm_char.prompt)
                dm_name = dm_char.name

            messages = [
                {"role": "system", "content": dm_prompt},
                {"role": "user", "content": f"请根据以下要求生成一条公告:\n{content}"}
            ]
            generated_content = chat_with_deepseek(messages, character_name=dm_name, color_code=TermColors.MAGENTA)
            if generated_content:
                final_content = generated_content
            else:
                log_error("公告生成失败。")
                return True

        location = params.get('Location', 'popup')
        self.ui.display_system_message(f"--- [{location.upper()}] 公告 ---\n{final_content}\n--------------------", color=TermColors.MAGENTA)
        self.state.add_dialogue_history('Notice', location=location, content=final_content)
        return True

    def _handle_player_notice(self, params: Dict, content: str) -> bool:
        """处理仅玩家可见的系统提示，不计入对话历史。"""
        self.ui.display_system_message(f"[系统提示]: {content}", color=TermColors.BLUE)
        return True

    def _handle_system_action(self, params: Dict, content: str) -> bool:
        """处理后台系统动作，通常是调用LLM生成内容并存入变量。"""
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
                # ※※※※※※※※修改这里可以更改定包含的历史记录数量※※※※※※※※
                history_count = config.LLM_CONVERSATION_HISTORY_LIMIT 
                formatted_history_lines = []
                player_name = self.state.session.player.name or "玩家"

                for record in self.state.dialogue_history[-history_count:]:
                    record_content = record.get('content')
                    if not record_content and 'data' in record and isinstance(record['data'], dict):
                        record_content = record['data'].get('content')
                    
                    if not record_content:
                        continue

                    line = ""
                    record_type = record.get('type')

                    if record_type == 'Dialogue':
                        char_id = record.get('data', {}).get('character_id')
                        character = self.state.session.characters.get(char_id)
                        if character:
                            line = f"{character.name}: {record_content}"
                    elif record_type == 'Player':
                        line = f"{player_name}: {record_content}"
                    elif record_type == 'Narration':
                        line = f"旁白: {record_content}"
                    
                    if line:
                        formatted_history_lines.append(line.strip())

                if formatted_history_lines:
                    history_text_block = "\n".join(formatted_history_lines)
                    final_user_prompt = (
                        "以下是到目前为止的对话历史记录，请将其作为背景参考。历史记录中的发言者已经用前缀标明。\n"
                        "--- 历史消息开始 ---\n"
                        f"{history_text_block}\n"
                        "--- 历史消息结束 ---\n\n"
                        "历史记录仅供参考。现在，请严格按照下面的指示完成你的任务：\n\n"
                        f"{content}"
                    )
                    log_debug(f"构建的带历史的Prompt: {final_user_prompt[:250]}...")
                else:
                    log_debug("SystemAction: 历史记录为空，不附加历史上下文。")
            
            system_prompt = "你是一个富有创造力的游戏剧本助手。请根据以下要求完成任务，并直接输出结果，不要包含任何额外解释。"
            
            messages = [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": final_user_prompt}
            ]

            generated_content = chat_with_deepseek(messages, character_name="幕后导演", is_internal_thought=True)
            if generated_content:
                self.state.set_variable(var_name, generated_content.strip())
                log_debug(f"SystemAction 执行完毕, 变量 '{var_name}' 已设置。")
            else:
                log_error(f"SystemAction 未能从LLM生成内容，变量 '{var_name}' 未设置。")
        else:
            log_error(f"未知的 SystemAction Tool: {tool}")
            
        # 注意：后台动作不计入对话历史
        return True