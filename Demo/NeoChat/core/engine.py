from datetime import datetime
from .models import GameSession
from .state_manager import StateManager
from .event_handler import EventHandler
from .ui import ConsoleUI
from .save_manager import SaveManager
from .llm_interface import chat_with_deepseek
from .logger import log_info, log_error, log_debug, log_info_color, log_warning, TermColors

import config

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
                    break
            else:
                self._process_end_condition()
                break

# 1) 让 _process_end_condition 支持传入 end_data，并且只读处理
    def _process_end_condition(self, end_data: dict | None = None):
        """处理剧情单元的结束条件；不再修改原始 StoryUnit，避免条件被永久写死。"""
        unit = self.state.get_current_story_unit()
        if end_data is None:
            if not unit or not unit.end_condition:
                log_info("剧情单元结束，无EndCondition，游戏结束。")
                self.game_over = True
                return
            end_data = unit.end_condition

        end_data = self.state.format_string(end_data)
        end_type = end_data.get('Type')
        log_debug(f"处理EndCondition: {end_type}")

        if end_type == 'Linear':
            self.state.transition_to_unit(end_data.get('NextUnitID'))

        elif end_type in ['FreeTime', 'LimitedFreeTime']:
            self.state.progress.runtime_state = 'InFreeTime'
            self.state.progress.context['free_time_config'] = end_data
            self.state.progress.context['turns_taken'] = 0
            self.ui.display_system_message(
                end_data.get('InstructionToPlayer', '进入自由活动时间。'),
                color=TermColors.BLUE
            )

        elif end_type == 'Branching':
            method = end_data.get('Method')
            if method == 'PlayerChoice':
                self.state.progress.runtime_state = 'WaitingForPlayerChoice'
                self.state.progress.context['choices_config'] = end_data.get('Branches', {})
            elif method == 'AIChoice':
                self._execute_ai_choice(end_data)
            else:
                log_error(f"未知的 Branching Method: {method}")
                self.game_over = True

        elif end_type == 'Conditional':
            found_match = False
            for case in end_data.get('Cases', []):
                if self.state.evaluate_condition(case['Condition']):
                    self._process_end_condition(case['Then'])
                    found_match = True
                    break
            if not found_match and 'Else' in end_data:
                self._process_end_condition(end_data['Else'])

        else:
            log_error(f"未知的EndCondition类型: {end_type}")
            self.game_over = True


    def _process_end_condition_recursively(self, end_data):
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
                # 如果是系统命令，重新显示选项并等待
                self.ui.display_choices(display_choices)
                continue

            if user_choice in choices_data:
                next_unit_id = choices_data[user_choice]['NextUnitID']
                self.state.transition_to_unit(next_unit_id)
                break
            else:
                self.ui.display_system_message("无效的选择，请重新输入。", color=TermColors.RED)

    def _handle_free_time(self):
        """处理自由活动时间的玩家输入和AI回应。"""
        free_time_config_data = self.state.progress.context.get('free_time_config')
        user_input = self.ui.prompt_for_input()

        if self._handle_system_commands(user_input):
            return

        exit_prompt = free_time_config_data.get('ExitPromptInInputBox', '')
        if exit_prompt and exit_prompt in user_input:
            log_info("检测到退出语，自由时间结束。")
            self.state.transition_to_unit(free_time_config_data['NextUnitID'])
            return

        final_player_input = user_input if user_input.strip() else "..."
        if final_player_input == "...":
            self.ui.display_player_dialogue(self.state.session.player.name, final_player_input)
            log_debug("玩家输入为空，已将其解释为 '...' (沉默)。")
        
        self.state.add_dialogue_history('Player', content=final_player_input)
        
        interact_with_list = free_time_config_data.get('InteractWith', [])
        if not interact_with_list:
            interact_with_list = list(self.state.session.characters.keys())
        
        if not interact_with_list:
            log_warning("自由时间模式下没有可互动的AI角色。")
            return

        # 根据配置决定对话顺序模式
        dialogue_order_mode = free_time_config_data.get('DialogueOrder', 'RoundRobin')

        responder_ids_in_order = []
        if dialogue_order_mode == 'Auto' and len(interact_with_list) > 1:
            log_info_color("AI 正在判断由谁来回应...", TermColors.MAGENTA)
            responder_ids_in_order = self._decide_auto_response_order(interact_with_list)
        else: # 默认RoundRobin模式，或只有一个角色
            last_responder_index = self.state.progress.context.get('last_responder_index', -1)
            next_responder_index = (last_responder_index + 1) % len(interact_with_list)
            responder_ids_in_order.append(interact_with_list[next_responder_index])
            self.state.progress.context['last_responder_index'] = next_responder_index

        # 循环让决定好的角色依次发言
        for responder_id in responder_ids_in_order:
            responder = self.state.session.characters.get(responder_id)
            if responder:
                log_info_color(f"现在由 {responder.name} 来回应...", TermColors.BLUE)
                messages = [{"role": "system", "content": self.state.format_string(responder.prompt)}]
                history_count = config.LLM_CONVERSATION_HISTORY_LIMIT
                for record in self.state.dialogue_history[-history_count:]:
                    record_content = record.get('content') or record.get('data', {}).get('content')
                    if not record_content: continue

                    record_type = record.get('type')
                    if record_type == 'Dialogue':
                        char_id = record.get('data', {}).get('character_id')
                        role = "assistant" if char_id == responder_id else "user"
                        messages.append({"role": role, "content": record_content})
                    elif record_type == 'Player':
                        messages.append({"role": "user", "content": record_content})
                    elif record_type == 'Narration':
                        messages.append({"role": "user", "content": f"（旁白：{record_content}）"})

                response = chat_with_deepseek(messages, responder.name, color_code=TermColors.CYAN)
                if response:
                    self.state.add_dialogue_history('Dialogue', character_id=responder_id, content=response)

        # 检查回合数限制
        self.state.progress.context['turns_taken'] += 1
        if free_time_config_data['Type'] == 'LimitedFreeTime' and self.state.progress.context['turns_taken'] >= free_time_config_data['MaxTurns']:
            log_info("达到最大轮次，自由时间结束。")
            self.state.transition_to_unit(free_time_config_data['NextUnitID'])

    def _decide_auto_response_order(self, participant_ids: list[str]) -> list[str]:
        """使用LLM决定在自由对话中，接下来应该由哪个或哪些AI角色按什么顺序回应。"""
        # 1. 构建角色信息
        character_profiles = []
        player_profile = f"ID: Player, 姓名: {self.state.session.player.name}, 人设: {self.state.session.player.prompt or '玩家自己'}"
        character_profiles.append(player_profile)

        for char_id in participant_ids:
            char = self.state.session.characters.get(char_id)
            if char:
                profile = f"ID: {char_id}, 姓名: {char.name}, 人设: {self.state.format_string(char.prompt)}"
                character_profiles.append(profile)
        
        # 2. 构建对话历史
        history_lines = []
        history_count = config.LLM_FREE_TIME_HISTORY_LIMIT
        for record in self.state.dialogue_history[-history_count:]:
            record_content = record.get('content') or record.get('data', {}).get('content')
            if not record_content: continue
            
            line = ""
            record_type = record.get('type')
            if record_type == 'Dialogue':
                char_id = record.get('data', {}).get('character_id')
                char_name = "未知角色"
                character = self.state.session.characters.get(char_id)
                if character:
                    char_name = character.name
                line = f"{char_name}: {record_content}"
            elif record_type == 'Player':
                line = f"{self.state.session.player.name}: {record_content}"
            
            if line:
                history_lines.append(line.strip())

        # 3. 构建完整的System Prompt
        system_prompt = (
            "你是一个对话流程控制器和社交观察AI。你的任务是根据当前对话的上下文、人物性格，来决定在玩家发言后，接下来应该由哪个或哪些AI角色进行回应，以及他们的回应顺序。"
            "你的决策必须合乎逻辑和情理。例如：\n"
            "- 如果玩家直接向某个角色提问，那个角色应该最先回应。\n"
            "- 如果一个角色长时间没有发言，而当前话题又与他相关，他可能会主动插话。\n"
            "- 可能会有多个角色同时对玩家的话产生反应，他们可能都发言。\n"
            f"你需要从以下AI角色中选择：{', '.join(participant_ids)}。\n"
            "你的输出必须极为简洁，只能包含你决定要回应的角色的ID，并按照回应的先后顺序排列，中间不加任何分隔符。例如，如果决定让角色'Yuki'先说，然后是'Aki'，你就应该只输出 'YukiAki'。如果只让'Aki'说，就输出 'Aki'。绝对不要包含任何解释、理由或额外文字。三个角色以及以上也以此类推。"
            "玩家看起来是在和谁互动就谁先回应，没有明显互动的角色可以不回应。除非该角色长时间没有发言。尽量只减少都回应的可能，挑选一些人来回应"

        )
        
        # 4. 构建User Prompt
        user_prompt = (
            "--- 角色信息 ---\n"
            f"{'\n'.join(character_profiles)}\n\n"
            "--- 最近对话历史 (最后一句是玩家刚刚的发言) ---\n"
            f"{'\n'.join(history_lines)}\n\n"
            "--- 任务 ---\n"
            f"System: 根据以上所有信息，决定接下来由谁（从 {', '.join(participant_ids)} 中选择）以何种顺序回应。\n"
            "请严格按照格式要求输出角色ID序列。玩家看起来是在和谁互动就谁先回应，没有明显互动的角色可以不回应。除非该角色长时间没有发言。尽量只减少都回应的可能，挑选一些人来回应"
        )
        
        messages = [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ]

        response_str = chat_with_deepseek(messages, character_name="对话控制器", is_internal_thought=True)
        if not response_str:
            log_warning("AI未能决定对话顺序，将使用默认轮询。")
            return [participant_ids[0]]

        # 5. 解析LLM的输出
        response_str = response_str.strip()
        ordered_responders = []
        temp_response_str = response_str
        
        # 使用贪婪匹配来解析ID序列，防止ID包含关系导致解析错误（例如 'A' 和 'AB'）
        sorted_ids = sorted(participant_ids, key=len, reverse=True)
        
        while temp_response_str:
            found_match = False
            for char_id in sorted_ids:
                if temp_response_str.startswith(char_id):
                    if char_id not in ordered_responders:
                        ordered_responders.append(char_id)
                    temp_response_str = temp_response_str[len(char_id):]
                    found_match = True
                    break
            if not found_match:
                log_warning(f"无法解析AI返回的对话顺序中的剩余部分: '{temp_response_str}'。原始回复: '{response_str}'")
                break

        log_debug(f"AI决定的对话顺序为: {ordered_responders}")
        return ordered_responders if ordered_responders else [participant_ids[0]]

    # --- 函数已修改 ---
    def _execute_ai_choice(self, config_data: dict):
        """执行 AI 决策逻辑。"""
        log_info_color("AI 正在做出决定...", TermColors.BLUE)
        decider_id = config_data['DeciderCharacterID']
        decider = self.state.session.characters.get(decider_id)
        if not decider:
            log_error(f"AI Choice 失败：找不到决策角色 '{decider_id}'。")
            self.game_over = True
            return

        # 1. 决策 Call
        decision_prompt = self.state.format_string(config_data['DecisionPromptForAI'])
        messages = [{"role": "system", "content": self.state.format_string(decider.prompt)}]
        messages.append({"role": "system", "content": decision_prompt})
        ai_decision_text = chat_with_deepseek(messages, character_name=f"{decider.name}(内心)", is_internal_thought=True)
        if not ai_decision_text:
            log_error("AI 未能做出决策。")
            self.game_over = True
            return
        log_debug(f"AI 决策原文: {ai_decision_text}")

        # 2. 判断 Call (已修改)
        history_context_str = "无"
        history_lines = []
        history_count = config.AI_CHOICE_HISTORY_CONTEXT_COUNT
        for record in self.state.dialogue_history[-history_count:]:
            record_content = record.get('content') or record.get('data', {}).get('content')
            if not record_content: continue
            
            line = ""
            record_type = record.get('type')
            if record_type == 'Dialogue':
                char_id = record.get('data', {}).get('character_id')
                char_name = "未知角色"
                character = self.state.session.characters.get(char_id)
                if character:
                    char_name = character.name
                line = f"{char_name}: {record_content}"
            elif record_type == 'Player':
                line = f"{self.state.session.player.name}: {record_content}"
            elif record_type == 'Narration':
                line = f"旁白: {record_content}"
            
            if line:
                history_lines.append(line.strip())
        
        if history_lines:
            history_context_str = "\n".join(history_lines)

        # 2.2 构建包含历史上下文的判断Prompt
        judge_prompt = self.state.format_string(config_data['JudgePromptForSystem'])
        judge_user_prompt = (
            "请结合以下最近的对话历史和AI角色的内心决策，进行最终判断。\n\n"
            "--- 最近对话历史 ---\n"
            f"{history_context_str}\n"
            "--- 对话历史结束 ---\n\n"
            "--- AI角色的内心决策 ---\n"
            f"{ai_decision_text}\n"
            "--- 内心决策结束 ---\n\n"
            "任务：请严格根据以上所有信息进行判断。"
        )

        judge_messages = [
            {"role": "system", "content": judge_prompt},
            {"role": "user", "content": judge_user_prompt}
        ]
        judged_result = chat_with_deepseek(judge_messages, character_name="系统判断", is_internal_thought=True)
        if not judged_result:
            log_error("系统未能判断 AI 的决策。")
            self.game_over = True
            return

        # 3. 转换剧情
        final_choice = judged_result.strip().upper()
        log_info_color(f"AI 的选择已被系统判断为: '{final_choice}'", TermColors.GREEN)
        
        branches = config_data.get('Branches', {})
        if final_choice in branches:
            self.state.transition_to_unit(branches[final_choice]['NextUnitID'])
        else:
            log_error(f"判断结果 '{final_choice}' 无效，在 Branches 中找不到匹配项。")
            self.game_over = True

    def _execute_player_response_branch(self, config_data: dict):
        """
        处理基于玩家回应的分支逻辑。
        在玩家输入后触发，使用LLM判断玩家的回应并选择分支。
        """
        log_info_color("AI 正在分析你的回应...", TermColors.BLUE)
        
        decider_id = config_data.get('DeciderCharacterID')
        decider = self.state.session.characters.get(decider_id) if decider_id else None
        
        # 1. 构建历史上下文
        history_lines = []
        history_count = config.AI_CHOICE_HISTORY_CONTEXT_COUNT
        for record in self.state.dialogue_history[-history_count:-1]: # 获取直到玩家输入前的历史
            record_content = record.get('content') or record.get('data', {}).get('content')
            if not record_content: continue
            
            line = ""
            record_type = record.get('type')
            if record_type == 'Dialogue':
                char_id = record.get('data', {}).get('character_id')
                char_name = self.state.session.characters.get(char_id, "未知角色").name
                line = f"{char_name}: {record_content}"
            elif record_type == 'Player':
                line = f"{self.state.session.player.name}: {record_content}"
            elif record_type == 'Narration':
                line = f"旁白: {record_content}"
            if line:
                history_lines.append(line.strip())
        
        history_context_str = "\n".join(history_lines) if history_lines else "无"
        player_response = self.state.runtime_context.get('player_input', '')

        # 2. 构建判断Prompt
        judge_system_prompt = self.state.format_string(config_data['JudgePromptForSystem'])
        
        # 如果定义了决策角色，将角色的prompt作为额外的系统提示注入，帮助LLM更好地“扮演”
        if decider:
            judge_system_prompt = f"请站在角色 {decider.name} 的角度思考，他/她的人设是：\n{self.state.format_string(decider.prompt)}\n\n---\n\n{judge_system_prompt}"

        judge_user_prompt = (
            "请结合以下最近的对话历史和玩家的最终回应，进行判断。\n\n"
            "--- 最近对话历史 ---\n"
            f"{history_context_str}\n"
            "--- 对话历史结束 ---\n\n"
            "--- 玩家的最终回应 ---\n"
            f"{self.state.session.player.name}: {player_response}\n"
            "--- 玩家回应结束 ---\n\n"
            "任务：请严格根据以上所有信息，并遵循你的系统指令进行判断。"
        )

        judge_messages = [
            {"role": "system", "content": judge_system_prompt},
            {"role": "user", "content": judge_user_prompt}
        ]

        # 3. 调用LLM进行判断
        judged_result = chat_with_deepseek(judge_messages, character_name="系统判断", is_internal_thought=True)
        if not judged_result:
            log_error("系统未能判断玩家的回应。")
            self.game_over = True
            return

        # 4. 转换剧情
        final_choice = judged_result.strip().upper()
        log_info_color(f"你的回应已被系统判断为: '{final_choice}'", TermColors.GREEN)
        
        branches = config_data.get('Branches', {})
        if final_choice in branches:
            self.state.transition_to_unit(branches[final_choice]['NextUnitID'])
        else:
            log_error(f"判断结果 '{final_choice}' 无效，在 Branches 中找不到匹配项。")
            self.game_over = True

    def _handle_system_commands(self, user_input: str) -> bool:
        """处理系统命令，如 /save。返回 True 表示已处理，无需后续操作。"""
        if user_input.lower().startswith('/save'):
            save_name = user_input[5:].strip() or f"手动存档_{datetime.now().strftime('%H%M%S')}"
            self.save_manager.save_game_session(self.session, save_name)
            return True
        return False