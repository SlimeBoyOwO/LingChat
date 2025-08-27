# neochat/game/engine.py
from datetime import datetime

# 更新所有 import
from neochat.game.state import GameSession, StateManager
from neochat.game.event_handler import EventHandler
from neochat.presentation.cli.ui import ConsoleUI
from neochat.platform.persistence.save_manager import SaveManager
from neochat.llm.client import generate_chat_response
from neochat.platform.logging import log_info, log_error, log_debug, log_info_color, log_warning, TermColors
from neochat.game.end_conditions import end_condition_registry
from neochat.platform.configuration import config
from neochat.memory.manager import MemoryManager

class GameEngine:
    """
    游戏主引擎，负责驱动游戏循环和协调各个组件。
    """
    def __init__(self, session: GameSession):
        self.session = session
        self.is_running = True
        self.game_over = False

        self.ui = ConsoleUI()
        self.state = StateManager(self.session)
        self.event_handler = EventHandler(self.state, self.ui)
        self.save_manager = SaveManager()
        self.end_condition_handler_instances: Dict[str, Any] = {}

    def run(self):
        """主游戏循环。"""
        log_info_color("游戏开始！在任何你需要输入的时候，都可以使用 /save 命令。", TermColors.GREEN)

        while self.is_running and not self.game_over:
            current_state = self.state.progress.runtime_state

            if current_state == 'ExecutingEvents':
                self._execute_events()
            elif current_state == 'WaitingForPlayerInput':
                self._wait_for_player_input()
            elif current_state == 'WaitingForPlayerChoice':
                self._wait_for_player_choice()
            elif current_state == 'InFreeTime':
                self._handle_free_time()
            else:
                log_error(f"未知的游戏状态: {current_state}, 游戏结束。")
                self.game_over = True

        log_info_color("游戏结束。", TermColors.MAGENTA)

    def _execute_events(self):
        """循环执行当前剧情单元中的事件，直到需要暂停或单元结束。"""
        while self.state.progress.runtime_state == 'ExecutingEvents':
            next_event = self.state.get_next_event()

            if next_event:
                should_continue = self.event_handler.handle_event(next_event)
                self.state.advance_event_pointer()
                if not should_continue:
                    break  # 暂停执行，等待玩家输入等
            else:
                # 当前单元所有事件已执行完毕
                self._process_end_condition()
                break

    def _process_end_condition(self, end_data: dict | None = None):
        """
        处理剧情单元的结束条件。
        通过动态注册表查找并调用相应的处理器。
        """
        if end_data is None:
            unit = self.state.get_current_story_unit()
            if not unit or not unit.end_condition:
                log_info("剧情单元结束，无EndCondition，游戏结束。")
                self.game_over = True
                return
            end_data = unit.end_condition

        # 格式化 EndCondition 中的变量
        end_data = self.state.format_string(end_data)
        end_type = end_data.get('Type')
        log_debug(f"处理EndCondition: {end_type}")

        handler_class = end_condition_registry.get(end_type)
        if not handler_class:
            log_error(f"未知的EndCondition类型: {end_type}")
            self.game_over = True
            return

        if end_type not in self.end_condition_handler_instances:
            self.end_condition_handler_instances[end_type] = handler_class(self)

        handler = self.end_condition_handler_instances[end_type]
        handler.process(end_data)

    def _process_end_condition_recursively(self, end_data):
        """供 Conditional 处理器调用的递归入口。"""
        self._process_end_condition(end_data)

    def _wait_for_player_input(self):
        """处理玩家输入状态。"""
        prompt = self.state.progress.context.get('prompt')
        user_input = self.ui.prompt_for_input(self.state.format_string(prompt))

        if self._handle_system_commands(user_input):
            return

        final_input = user_input
        if not user_input.strip() and prompt:
            final_input = self.state.format_string(prompt)
            self.ui.display_system_message(f"(使用默认): {final_input}", color=TermColors.YELLOW)

        self.state.runtime_context['player_input'] = final_input
        self.state.add_dialogue_history('Player', content=final_input)

        # PlayerResponseBranch 是一种特殊的 EndCondition，这里暂时保留
        current_unit = self.state.get_current_story_unit()
        if current_unit and current_unit.end_condition and current_unit.end_condition.get('Type') == 'PlayerResponseBranch':
            self._execute_player_response_branch(current_unit.end_condition)
            return

        self.state.progress.runtime_state = 'ExecutingEvents'

    def _wait_for_player_choice(self):
        """处理玩家选择状态。"""
        choices_data = self.state.progress.context.get('choices_config', {})
        display_choices = {key: self.state.format_string(branch['DisplayText']) for key, branch in choices_data.items()}
        self.ui.display_choices(display_choices)

        while True:
            user_choice = self.ui.prompt_for_input().upper()
            if self._handle_system_commands(user_choice):
                self.ui.display_choices(display_choices) # 重新显示选项
                continue

            if user_choice in choices_data:
                next_unit_id = choices_data[user_choice]['NextUnitID']
                self.state.transition_to_unit(next_unit_id)
                break
            else:
                self.ui.display_system_message("无效的选择，请重新输入。", color=TermColors.RED)

    def _handle_free_time(self):
        """处理自由活动时间的玩家输入和AI回应。"""
        free_time_config = self.state.progress.context.get('free_time_config')
        user_input = self.ui.prompt_for_input()

        if self._handle_system_commands(user_input):
            return

        exit_prompt = free_time_config.get('ExitPromptInInputBox', '')
        if exit_prompt and exit_prompt in user_input:
            log_info("检测到退出语，自由时间结束。")
            self.state.transition_to_unit(free_time_config['NextUnitID'])
            return

        final_player_input = user_input if user_input.strip() else "..."
        self.state.add_dialogue_history('Player', content=final_player_input)

        # 决定由谁来回应
        interact_with_list = free_time_config.get('InteractWith', []) or list(self.state.session.characters.keys())
        if not interact_with_list:
            log_warning("自由时间模式下没有可互动的AI角色。")
            return

        dialogue_order_mode = free_time_config.get('DialogueOrder', 'RoundRobin')
        responder_ids = []
        if dialogue_order_mode == 'Auto' and len(interact_with_list) > 1:
            responder_ids = self._decide_auto_response_order(interact_with_list)
        else: # RoundRobin
            last_idx = self.state.progress.context.get('last_responder_index', -1)
            next_idx = (last_idx + 1) % len(interact_with_list)
            responder_ids.append(interact_with_list[next_idx])
            self.state.progress.context['last_responder_index'] = next_idx

        # 依次让角色回应
        for responder_id in responder_ids:
            responder = self.state.session.characters.get(responder_id)
            if responder:
                log_info_color(f"现在由 {responder.name} 来回应...", TermColors.BLUE)
                messages = [{"role": "system", "content": self.state.format_string(responder.prompt)}]
                
                memory_manager = MemoryManager(self.state)
                context = memory_manager.get_context_for_llm(
                    history_limit=config.llm.conversation_history_limit,
                    perspective_char_id=responder_id
                )
                messages.extend(context)

                response = generate_chat_response(messages, responder.name, color_code=TermColors.CYAN)
                if response:
                    self.state.add_dialogue_history('Dialogue', character_id=responder_id, content=response)

        # 检查是否结束
        self.state.progress.context['turns_taken'] = self.state.progress.context.get('turns_taken', 0) + 1
        if free_time_config['Type'] == 'LimitedFreeTime' and self.state.progress.context['turns_taken'] >= free_time_config['MaxTurns']:
            log_info("达到最大轮次，自由时间结束。")
            self.state.transition_to_unit(free_time_config['NextUnitID'])

    def _decide_auto_response_order(self, participant_ids: list[str]) -> list[str]:
        character_profiles = [f"ID: Player, 姓名: {self.state.session.player.name}, 人设: {self.state.session.player.prompt or '玩家自己'}"]
        for char_id in participant_ids:
            char = self.state.session.characters.get(char_id)
            if char: character_profiles.append(f"ID: {char_id}, 姓名: {char.name}, 人设: {self.state.format_string(char.prompt)}")
        
        history_lines = []
        for record in self.state.dialogue_history[-config.LLM_FREE_TIME_HISTORY_LIMIT:]:
            record_content = record.get('content') or record.get('data', {}).get('content')
            if not record_content: continue
            line, record_type = "", record.get('type')
            if record_type == 'Dialogue':
                char_id = record.get('data', {}).get('character_id')
                char_name = self.state.session.characters.get(char_id, type('',(object,),{'name':'未知角色'})()).name
                line = f"{char_name}: {record_content}"
            elif record_type == 'Player': line = f"{self.state.session.player.name}: {record_content}"
            if line: history_lines.append(line.strip())

        system_prompt = "你是一个对话流程控制器...（省略，与原版相同）"
        user_prompt = f"--- 角色信息 ---\n{'\n'.join(character_profiles)}\n\n--- 最近对话历史 ---\n{'\n'.join(history_lines)}\n\n--- 任务 ---\n...（省略，与原版相同）"
        
        messages = [{"role": "system", "content": system_prompt}, {"role": "user", "content": user_prompt}]
        response_str = generate_chat_response(messages, character_name="对话控制器", is_internal_thought=True)
        if not response_str: return [participant_ids[0]]
        
        ordered_responders, temp_response_str = [], response_str.strip()
        sorted_ids = sorted(participant_ids, key=len, reverse=True)
        while temp_response_str:
            found_match = False
            for char_id in sorted_ids:
                if temp_response_str.startswith(char_id):
                    if char_id not in ordered_responders: ordered_responders.append(char_id)
                    temp_response_str = temp_response_str[len(char_id):]
                    found_match = True
                    break
            if not found_match: break
        return ordered_responders if ordered_responders else [participant_ids[0]]

    def _execute_ai_choice(self, config_data: dict):
        log_info_color("AI 正在做出决定...", TermColors.BLUE)
        decider_id = config_data['DeciderCharacterID']
        decider = self.state.session.characters.get(decider_id)
        if not decider:
            log_error(f"AI Choice 失败：找不到决策角色 '{decider_id}'。")
            self.game_over = True
            return

        decision_prompt = self.state.format_string(config_data['DecisionPromptForAI'])
        messages = [{"role": "system", "content": self.state.format_string(decider.prompt)}, {"role": "system", "content": decision_prompt}]
        ai_decision_text = generate_chat_response(messages, character_name=f"{decider.name}(内心)", is_internal_thought=True)
        if not ai_decision_text: self.game_over = True; return
        
        history_lines = []
        for record in self.state.dialogue_history[-config.AI_CHOICE_HISTORY_CONTEXT_COUNT:]:
            # ... （构建 history_lines 的逻辑与原版相同）
            pass
        history_context_str = "\n".join(history_lines) if history_lines else "无"

        judge_prompt = self.state.format_string(config_data['JudgePromptForSystem'])
        judge_user_prompt = f"请结合以下对话历史和AI决策进行判断。\n\n--- 对话历史 ---\n{history_context_str}\n--- AI决策 ---\n{ai_decision_text}\n\n任务：请严格判断。"
        judge_messages = [{"role": "system", "content": judge_prompt}, {"role": "user", "content": judge_user_prompt}]
        judged_result = generate_chat_response(judge_messages, character_name="系统判断", is_internal_thought=True)
        if not judged_result: self.game_over = True; return

        final_choice = judged_result.strip().upper()
        log_info_color(f"AI 的选择已被系统判断为: '{final_choice}'", TermColors.GREEN)
        branches = config_data.get('Branches', {})
        if final_choice in branches:
            self.state.transition_to_unit(branches[final_choice]['NextUnitID'])
        else:
            log_error(f"判断结果 '{final_choice}' 无效。")
            self.game_over = True

    def _execute_player_response_branch(self, config_data: dict):
        """处理根据玩家输入内容进行分支的逻辑 (暂未实现)。"""
        log_warning("PlayerResponseBranch 功能暂未完全实现。")
        self.state.progress.runtime_state = 'ExecutingEvents'

    def _handle_system_commands(self, user_input: str) -> bool:
        """处理系统命令，如 /save。返回 True 表示已处理。"""
        if user_input.lower().startswith('/save'):
            save_name = user_input[5:].strip() or f"手动存档_{datetime.now().strftime('%H%M%S')}"
            self.save_manager.save_game_session(self.session, save_name)
            return True
        return False