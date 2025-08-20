# game_engine.py
import os
import shutil
import yaml
import re
import random
import uuid
from datetime import datetime

import config
from .logger import log_debug, log_info, log_warning, log_error, log_info_color, TermColors
from core.llm_interface import chat_with_deepseek

class GameEngine:
    def __init__(self):
        self.save_path = None
        self.story_pack_path = None
        self.character_files = {} # role_id -> character_data
        
        # In-memory state
        self.game_state = {}
        self.progress = {}
        self.dialogue_history = []
        self.global_config = {}
        self.current_story_unit = None
        # --- 新增: 运行时上下文，用于存放不需存档的临时变量 ---
        self.runtime_context = {}

        self.is_running = False
        self.game_over = False

    def _format_string(self, text):
        """用 game_state 和 runtime_context 中的变量替换字符串中的 {placeholder}"""
        if not isinstance(text, str):
            return text
        
        # 正则表达式寻找 {variable_name}
        placeholders = re.findall(r'\{([a-zA-Z0-9_]+)\}', text)
        
        formatted_text = text
        for placeholder in placeholders:
            value = None
            # --- 核心修改：优先从运行时上下文查找 ---
            if placeholder in self.runtime_context:
                value = self.runtime_context[placeholder]
            # --- 其次从游戏状态查找 ---
            elif placeholder in self.game_state:
                value = self.game_state[placeholder]
            else:
                log_warning(f"格式化字符串时未在 game_state 或 runtime_context 中找到变量: {placeholder}")
                continue # 如果找不到，跳过替换

            # 确保替换值的类型正确
            if value is not None:
                formatted_text = formatted_text.replace(f'{{{placeholder}}}', str(value))

        return formatted_text

    def _evaluate_condition(self, condition_str):
        """安全地评估条件表达式"""
        formatted_condition = self._format_string(condition_str)
        log_debug(f"正在评估条件: `{condition_str}` -> `{formatted_condition}`")
        try:
            # 为安全起见，只允许简单的比较和逻辑运算
            # 更安全的做法是使用ast.literal_eval或一个专门的表达式求值库
            result = eval(formatted_condition, {"__builtins__": {}}, {})
            log_debug(f"条件评估结果: {result}")
            return result
        except Exception as e:
            log_error(f"评估条件时出错: '{formatted_condition}'. 错误: {e}")
            return False

    def _add_to_dialogue_history(self, event_type, **kwargs):
        log_entry = {
            "id": f"evt_{uuid.uuid4()}",
            "timestamp": datetime.now().isoformat(),
            "source_unit_id": self.progress.get('progress_pointer', {}).get('current_unit_id'),
            "source_event_index": self.progress.get('progress_pointer', {}).get('last_completed_event_index'),
            "type": event_type
        }

        # 核心修改逻辑：根据kwargs的结构决定数据格式
        # 如果kwargs只有一个'content'键，则将其提升到顶层
        if len(kwargs) == 1 and 'content' in kwargs:
            log_entry['content'] = kwargs['content']
        # 否则，将所有kwargs作为结构化数据放入'data'字段
        else:
            log_entry['data'] = kwargs

        self.dialogue_history.append(log_entry)
        log_debug(f"添加新对话记录: {log_entry}")

    def load_story_pack(self, story_pack_path, character_selections, player_data=None):
        """开始一个新游戏"""
        try:
            # 1. 创建存档文件夹
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            self.save_path = os.path.join(config.SAVES_BASE_PATH, f"save_{timestamp}")
            os.makedirs(os.path.join(self.save_path, "save"), exist_ok=True)
            
            # 2. 复制剧本和角色文件
            shutil.copytree(story_pack_path, self.save_path, dirs_exist_ok=True)
            char_dir_in_save = os.path.join(self.save_path, "character")
            os.makedirs(char_dir_in_save, exist_ok=True)
            for role_id, char_path in character_selections.items():
                shutil.copy(char_path, os.path.join(char_dir_in_save, f"{role_id}.yaml"))

            self.story_pack_path = self.save_path
            
            # 3. 加载全局配置和角色
            with open(os.path.join(self.story_pack_path, '全局剧情配置.yaml'), 'r', encoding='utf-8') as f:
                self.global_config = yaml.safe_load(f)
            
            self._load_character_files()

            # 4. 初始化 gamestate.yaml
            with open(os.path.join(self.story_pack_path, 'save', 'gamestate.yaml'), 'r', encoding='utf-8') as f:
                self.game_state = yaml.safe_load(f) or {}

            # 核心修改: 如果有外部玩家数据，则覆盖 gamestate
            if player_data:
                log_debug(f"检测到外部玩家数据，正在更新 gamestate...")
                self.game_state['player_name'] = player_data.get('player_name', self.game_state.get('player_name', '未知玩家'))
                self.game_state['player_prompt'] = player_data.get('player_prompt', '')
                log_debug(f"玩家姓名已设置为: {self.game_state['player_name']}")
                log_debug(f"玩家设定已加载。")

            # 5. 初始化 游戏进度.yaml
            self.progress = {
                "save_name": "New Game",
                "story_pack_id": self.global_config.get('id', 'unknown'),
                "last_saved_timestamp": datetime.now().isoformat(),
                "progress_pointer": {
                    "current_unit_id": self.global_config['start_unit_id'],
                    "last_completed_event_index": -1
                },
                "runtime_state": "ExecutingEvents"
            }

            # 6. 初始化 对话记录.yaml
            self.dialogue_history = []
            
            self.is_running = True
            log_info(f"新游戏已创建，存档位于: {self.save_path}")
            
            # 进行一次初始保存，确保所有状态文件都已创建
            self.save_game("初始存档") 
            
            return True
        except Exception as e:
            log_error(f"创建新游戏失败: {e}")
            return False

    def _load_character_files(self):
        char_dir = os.path.join(self.story_pack_path, "character")
        self.character_files = {}
        for filename in os.listdir(char_dir):
            if filename.endswith(".yaml"):
                role_id = filename.split('.')[0]
                with open(os.path.join(char_dir, filename), 'r', encoding='utf-8') as f:
                    self.character_files[role_id] = yaml.safe_load(f)
                # 将角色名注入gamestate
                self.game_state[f'character_name_{role_id}'] = self.character_files[role_id]['name']

    def load_from_save(self, save_path):
        """从存档加载游戏"""
        try:
            self.save_path = save_path
            self.story_pack_path = save_path
            
            with open(os.path.join(self.save_path, 'save', 'gamestate.yaml'), 'r', encoding='utf-8') as f:
                self.game_state = yaml.safe_load(f)
            with open(os.path.join(self.save_path, 'save', '游戏进度.yaml'), 'r', encoding='utf-8') as f:
                self.progress = yaml.safe_load(f)
            with open(os.path.join(self.save_path, 'save', '对话记录.yaml'), 'r', encoding='utf-8') as f:
                self.dialogue_history = yaml.safe_load(f)
            with open(os.path.join(self.story_pack_path, '全局剧情配置.yaml'), 'r', encoding='utf-8') as f:
                self.global_config = yaml.safe_load(f)

            self._load_character_files()

            self.is_running = True
            log_info(f"成功从 {save_path} 加载存档。")
            return True
        except Exception as e:
            log_error(f"加载存档失败: {e}")
            return False

    def save_game(self, save_name=None):
        if not self.is_running:
            log_warning("游戏未运行，无法保存。")
            return
        
        try:
            if save_name:
                self.progress['save_name'] = save_name
            self.progress['last_saved_timestamp'] = datetime.now().isoformat()
            
            save_dir = os.path.join(self.save_path, 'save')
            with open(os.path.join(save_dir, 'gamestate.yaml'), 'w', encoding='utf-8') as f:
                yaml.dump(self.game_state, f, allow_unicode=True)
            with open(os.path.join(save_dir, '游戏进度.yaml'), 'w', encoding='utf-8') as f:
                yaml.dump(self.progress, f, allow_unicode=True)
            with open(os.path.join(save_dir, '对话记录.yaml'), 'w', encoding='utf-8') as f:
                yaml.dump(self.dialogue_history, f, allow_unicode=True)
            
            log_info_color(f"游戏已保存，存档名: '{self.progress['save_name']}'", TermColors.GREEN)
        except Exception as e:
            log_error(f"保存游戏失败: {e}")

    def run(self):
        """主游戏循环，执行事件直到需要玩家输入"""
        if not self.is_running or self.game_over:
            return

        while self.progress['runtime_state'] == 'ExecutingEvents' and not self.game_over:
            pointer = self.progress['progress_pointer']
            unit_id = pointer['current_unit_id']
            
            # 加载当前剧情单元
            unit_path = os.path.join(self.story_pack_path, 'story', f"{unit_id}.yaml")
            if not os.path.exists(unit_path):
                log_error(f"剧情单元文件未找到: {unit_path}")
                self.game_over = True
                break
            with open(unit_path, 'r', encoding='utf-8') as f:
                self.current_story_unit = yaml.safe_load(f)

            next_event_index = pointer['last_completed_event_index'] + 1
            events = self.current_story_unit.get('Events', [])
            
            if next_event_index >= len(events):
                # 事件执行完毕，处理EndCondition
                self._process_end_condition(self.current_story_unit.get('EndCondition'))
            else:
                # 执行下一个事件
                self._process_event(events[next_event_index])
                if self.progress['runtime_state'] == 'ExecutingEvents': # 确保事件没有改变状态
                     pointer['last_completed_event_index'] = next_event_index
        
        log_debug(f"引擎暂停，当前状态: {self.progress['runtime_state']}")


    def _process_event(self, event_data):
        log_debug(f"处理事件: {event_data}")

        # 处理条件
        if 'Condition' in event_data:
            if not self._evaluate_condition(event_data['Condition']):
                log_debug("条件不满足，跳过该事件块。")
                # 即使跳过，也算完成这个"条件事件"
                self.progress['progress_pointer']['last_completed_event_index'] += 1
                return
            # 如果条件满足，执行嵌套的Events
            for nested_event in event_data['Events']:
                 self._process_event(nested_event)
            # 完成后，更新主事件索引
            self.progress['progress_pointer']['last_completed_event_index'] += 1
            return

        event_key, event_content = list(event_data.items())[0]
        params = dict(param.strip().split(': ') for param in event_key.split(' | '))
        event_type = params['Type']
        
        # 格式化内容
        if isinstance(event_content, str):
            content = self._format_string(event_content)
        elif isinstance(event_content, dict):
            content = {k: self._format_string(v) for k, v in event_content.items()}
        else:
            content = event_content

        if event_type == 'Narration':
            if params.get('Mode') == 'Prompt':
                log_debug("生成 'Narration' prompt...")
                # 旁白/DM的通用系统提示
                narrator_prompt = "你是一个优秀的、沉浸式的故事讲述者（旁白）。请根据以下要求和对话历史，生成一段富有文采的旁白。直接输出旁白内容，不要包含任何额外解释。"
                messages = [
                    {"role": "system", "content": narrator_prompt},
                    {"role": "user", "content": f"这是你的生成要求：\n{content}"}
                ]
                # 可以选择性地加入最近的对话历史
                for record in self.dialogue_history[-5:]:
                    # 从记录中获取纯文本内容
                    content = record.get('content') or record.get('data', {}).get('content')
                    if not content:
                        continue

                    if record['type'] == 'Dialogue':
                        hist_char_id = record.get('data', {}).get('character_id')
                        if hist_char_id:
                            # 对于旁白生成，所有历史对话都作为用户输入
                            messages.insert(-1, {"role": "user", "content": content})
                    elif record['type'] == 'Player':
                        # 玩家发言也作为用户输入
                        messages.insert(-1, {"role": "user", "content": content})
                
                # --- 核心修改点 ---
                # chat_with_deepseek 内部已经处理了流式打印
                # 它返回的 generated_content 仅用于保存历史记录
                generated_content = chat_with_deepseek(messages, character_name="旁白", color_code=TermColors.GREY)
                
                if generated_content:
                    # 注意：这里不再执行 print()
                    self._add_to_dialogue_history('Narration', content=generated_content)
                else:
                    log_error("旁白生成失败。")
                    self.game_over = True
                    return
            else: # 这是处理 Mode: Preset 的情况
                print(f"{TermColors.GREY}旁白: {content}{TermColors.RESET}")
                self._add_to_dialogue_history('Narration', content=content)
        
        elif event_type == 'Dialogue':
            char_id = self._format_string(params['Character'])
            char_name = self.character_files.get(char_id, {}).get('name', char_id)
            
            if params['Mode'] == 'Preset':
                print(f"{TermColors.CYAN}{char_name}:{TermColors.RESET} {content}")
                self._add_to_dialogue_history('Dialogue', character_id=char_id, content=content)
            
            elif params['Mode'] == 'Prompt':
                # 构建LLM请求
                messages = []
                # 1. 添加角色设定
                character_prompt = self._format_string(self.character_files[char_id]['prompt'])
                messages.append({"role": "system", "content": character_prompt})
                
                # 2. 添加对话历史
                for record in self.dialogue_history[-10:]: # 取最近10条
                    content_text = record.get('content') or record.get('data', {}).get('content')
                    if not content_text:
                        continue

                    if record['type'] == 'Dialogue':
                        hist_char_id = record.get('data', {}).get('character_id')
                        if hist_char_id:
                            role = "assistant" if hist_char_id == char_id else "user"
                            messages.append({"role": role, "content": content_text})
                    elif record['type'] == 'Player':
                        messages.append({"role": "user", "content": content_text})
                    elif record['type'] == 'Narration':
                        messages.append({"role": "user", "content": f"（旁白：{content_text}）"})

                # --- 核心修复：直接使用 event_content，并赋值给新的、干净的变量 prompt_text ---
                prompt_text = self._format_string(event_content)
                
                # 3. 添加当前Prompt
                messages.append({"role": "user", "content": f"这是你的内心独白或行为指引，请根据它生成一句对话。不要把内心独白本身说出来。\n内心独白: {prompt_text}"})
                
                # 调用LLM
                response = chat_with_deepseek(messages, char_name, color_code=TermColors.CYAN)

                if response:
                    self._add_to_dialogue_history('Dialogue', character_id=char_id, content=response)
                else:
                    log_error("LLM未能生成响应，游戏可能无法继续。")
                    self.game_over = True
                
        elif event_type == 'Player':
            if params['Mode'] == 'Input':
                self.progress['runtime_state'] = 'WaitingForPlayerInput'
                if content: # 有默认提示
                    print(f"{TermColors.YELLOW}你 (可输入或直接回车使用默认): {content}{TermColors.RESET}")
                else:
                    print(f"{TermColors.YELLOW}你:{TermColors.RESET} ", end="")
            elif params['Mode'] == 'Preset':
                print(f"{TermColors.YELLOW}你:{TermColors.RESET} {content}")
                self._add_to_dialogue_history('Player', content=content)
        
        elif event_type == 'Notice':
            # 检查是否为 Prompt 模式
            if params.get('Mode') == 'Prompt':
                log_debug("生成 'Notice' prompt...")
                # 公告通常由DM发出，所以使用DM的人设
                dm_char_id = self.global_config.get('dm_role_id', 'DM')  # 假设全局配置中有DM角色ID
                dm_char = self.character_files.get(dm_char_id)
                # --- 修复点：格式化DM角色prompt中的模板变量 ---
                if dm_char:
                    dm_prompt = self._format_string(dm_char['prompt'])
                else:
                    dm_prompt = "你是一个剧本杀的DM（主持人）。"
                
                messages = [
                    {"role": "system", "content": dm_prompt},
                    {"role": "user", "content": f"这是你的生成要求：\n{content}"}
                ]
                # 可以加入对话历史
                for record in self.dialogue_history[-5:]:
                    # 从记录中获取纯文本内容
                    content = record.get('content') or record.get('data', {}).get('content')
                    if not content:
                        continue

                    if record['type'] == 'Dialogue':
                        hist_char_id = record.get('data', {}).get('character_id')
                        if hist_char_id:
                            # 对于公告生成，所有历史对话都作为用户输入
                            messages.insert(-1, {"role": "user", "content": content})
                    elif record['type'] == 'Player':
                        # 玩家发言也作为用户输入
                        messages.insert(-1, {"role": "user", "content": content})
                
                generated_content = chat_with_deepseek(messages, character_name=dm_char.get('name', 'DM') if dm_char else 'DM', color_code=TermColors.MAGENTA)
                if generated_content:
                    content = generated_content
                else:
                    log_error("公告生成失败。")
                    self.game_over = True
                    return

            location = params.get('Location', 'popup')
            print(f"\n{TermColors.MAGENTA}--- [{location.upper()}] 公告 ---\n{content}\n--------------------{TermColors.RESET}")
            self._add_to_dialogue_history('Notice', location=location, content=content)
        
        elif event_type == 'Chapter':
            print(f"\n{TermColors.GREEN}===== {content['Title']} ====={TermColors.RESET}")
            # 检查'Description'是否存在且不为空，如果存在才打印
            if content.get('Description'):
                print(f"{TermColors.GREY}{content['Description']}{TermColors.RESET}\n")
            self._add_to_dialogue_history('Chapter', **content)

        # --- 新增: 处理玩家可见、LLM不可见的通知 ---
        elif event_type == 'PlayerNotice':
            # 该事件仅向玩家显示信息，不会被记录到对话历史中，因此LLM无法感知
            log_debug(f"处理 PlayerNotice: {content}")
            print(f"{TermColors.BLUE}[系统提示]: {content}{TermColors.RESET}")
            # 注意：这里我们故意不调用 _add_to_dialogue_history

        # --- 新增: 处理LLM可见、玩家不可见的系统动作 ---
        elif event_type == 'SystemAction':
            # 该事件在后台调用LLM，并将结果存入变量，玩家看不到这个过程
            tool = params.get('Tool')
            var_name = params.get('Variable')
            if not tool or not var_name:
                log_error(f"SystemAction 事件缺少 Tool 或 Variable 参数: {params}")
                return

            if tool == 'Generate':
                log_info_color("AI 正在幕后构思剧情...", TermColors.MAGENTA)
                
                system_prompt = "你是一个富有创造力的游戏剧本助手。请根据以下要求完成任务，并直接输出结果，不要包含任何额外解释。"

                # 检查是否需要包含历史记录
                include_history = str(params.get('IncludeHistory', 'false')).lower() == 'true'
                final_user_prompt = content  # YAML中定义且已格式化的prompt

                if include_history:
                    log_debug("SystemAction: 检测到 IncludeHistory=true，正在构建历史上下文...")
                    history_count = 15
                    formatted_history_lines = []
                    player_name = self.game_state.get('player_name', '你')

                    for record in self.dialogue_history[-history_count:]:
                        record_content = record.get('content') or record.get('data', {}).get('content')
                        if not record_content:
                            continue

                        line = ""
                        if record['type'] == 'Dialogue':
                            char_id = record.get('data', {}).get('character_id')
                            char_name = self.character_files.get(char_id, {}).get('name', '未知角色')
                            line = f"{char_name}: {record_content}"
                        elif record['type'] == 'Player':
                            line = f"{player_name}: {record_content}"
                        elif record['type'] == 'Narration':
                            line = f"旁白: {record_content}"

                        if line:
                            formatted_history_lines.append(line)

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
                        log_debug(f"构建的带历史的Prompt: {final_user_prompt[:200]}...")
                    else:
                        log_debug("SystemAction: 历史记录为空，不附加历史上下文。")

                messages = [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": final_user_prompt}
                ]

                generated_content = chat_with_deepseek(
                    messages, 
                    character_name="幕后导演", 
                    is_internal_thought=True
                )

                if generated_content:
                    self.game_state[var_name] = generated_content.strip()
                    log_debug(f"SystemAction 执行完毕, 变量 '{var_name}' 已设置为AI生成的内容。")
                else:
                    log_error(f"SystemAction 未能从LLM生成内容，变量 '{var_name}' 未设置。")
            else:
                log_warning(f"未知的 SystemAction Tool: {tool}")
            
            # 注意：这里同样不调用 _add_to_dialogue_history，因为这是幕后行为

        elif event_type == 'Action':
            tool = params['Tool']
            var_name = params['Variable']
            if tool == 'Set':
                self.game_state[var_name] = content['Value']
            elif tool == 'Calculate':
                expr = self._format_string(content['Expression'])
                self.game_state[var_name] = eval(expr, {}, self.game_state)
            elif tool == 'Random':
                self.game_state[var_name] = random.randint(content['Min'], content['Max'])
            elif tool == 'RandomChoice':
                choices = content['Choices']
                self.game_state[var_name] = random.choice(choices)
            log_debug(f"Action执行完毕, {var_name} = {self.game_state.get(var_name)}")


    def _process_end_condition(self, end_data):
        if not end_data:
            log_info("剧情单元结束，无EndCondition，游戏结束。")
            self.game_over = True
            return

        end_type = end_data['Type']
        log_debug(f"处理EndCondition: {end_type}")

        if end_type == 'Linear':
            self._transition_to_unit(end_data['NextUnitID'])
        
        elif end_type == 'FreeTime' or end_type == 'LimitedFreeTime':
            self.progress['runtime_state'] = 'InFreeTime'
            self.progress['free_time_context'] = {
                'end_condition': end_data,
                'turns_taken': 0,
                # 新增：可选的指定互动角色列表（按顺序轮询）
                'interact_with_list': end_data.get('InteractWith')
            }
            log_info_color(self._format_string(end_data['InstructionToPlayer']), TermColors.BLUE)
        
        elif end_type == 'Branching':
            if end_data['Method'] == 'PlayerChoice':
                self.progress['runtime_state'] = 'WaitingForPlayerChoice'
                self.progress['choice_context'] = end_data
                print(f"{TermColors.YELLOW}请做出你的选择：{TermColors.RESET}")
                for key, branch in end_data['Branches'].items():
                    print(f"  [{key}] {self._format_string(branch['DisplayText'])}")
            
            elif end_data['Method'] == 'AIChoice':
                log_info_color("AI 正在做出决定...", TermColors.BLUE)
                self.progress['runtime_state'] = 'ProcessingAIChoice'
                self.progress['ai_choice_context'] = end_data
                self._execute_ai_choice()  # 直接调用处理函数

        elif end_type == 'Conditional':
            found_match = False
            for case in end_data['Cases']:
                if self._evaluate_condition(case['Condition']):
                    # 递归处理 'Then' 中的EndCondition
                    self._process_end_condition(case['Then'])
                    found_match = True
                    break
            if not found_match and 'Else' in end_data:
                self._process_end_condition(end_data['Else'])
        
        else:
            log_error(f"未知的EndCondition类型: {end_type}")
            self.game_over = True

    def _transition_to_unit(self, unit_id):
        log_debug(f"切换剧情单元到: {unit_id}")
        self.progress['progress_pointer']['current_unit_id'] = unit_id
        self.progress['progress_pointer']['last_completed_event_index'] = -1
        self.progress['runtime_state'] = 'ExecutingEvents'

    def provide_player_input(self, text):
        state = self.progress['runtime_state']
        if state == 'WaitingForPlayerInput':
            pointer = self.progress['progress_pointer']
            next_event_index = pointer['last_completed_event_index'] + 1
            events = self.current_story_unit.get('Events', [])
            event_data = events[next_event_index]
            _, event_content = list(event_data.items())[0]

            if not text.strip() and event_content: # 用户直接回车，使用默认值
                text = self._format_string(event_content)
                print(f"{TermColors.YELLOW}(使用默认): {text}{TermColors.RESET}")
            
            # --- 新增: 将玩家的输入存入运行时上下文 ---
            # 这使得紧随其后的事件（如Action:Set）可以使用 {player_input}
            self.runtime_context['player_input'] = text

            self._add_to_dialogue_history('Player', content=text)
            pointer['last_completed_event_index'] = next_event_index
            self.progress['runtime_state'] = 'ExecutingEvents'
            self.run() # 继续执行
        
        elif state == 'InFreeTime':
            context = self.progress['free_time_context']
            end_condition = context['end_condition']
            exit_prompt = self._format_string(end_condition.get('ExitPromptInInputBox', ''))

            if exit_prompt and exit_prompt in text:
                log_info("检测到退出语，自由时间结束。")
                self._transition_to_unit(end_condition['NextUnitID'])
                self.run()
                return

            # --- 新增: 将玩家的输入存入运行时上下文 ---
            self.runtime_context['player_input'] = text
            
            self._add_to_dialogue_history('Player', content=text)
            
            # AI 回复 - 使用智能轮询机制（支持 InteractWith 指定列表）
            characters_for_freetime = context.get('interact_with_list')

            ai_char_id = None
            if characters_for_freetime:
                # 过滤无效的角色ID
                valid_roles = [rid for rid in characters_for_freetime if rid in self.character_files]
                if len(valid_roles) != len(characters_for_freetime):
                    log_warning("InteractWith 中包含无效的角色ID，已自动忽略无效项。")
                characters_for_freetime = valid_roles

                if characters_for_freetime:
                    last_responder_index = self.progress.get('last_responder_index', -1)
                    next_responder_index = (last_responder_index + 1) % len(characters_for_freetime)
                    ai_char_id = characters_for_freetime[next_responder_index]
                    self.progress['last_responder_index'] = next_responder_index
                else:
                    log_warning("自由时间中没有可用的AI角色进行对话（InteractWith 过滤后为空）。")
                    return
            else:
                # 未指定列表：保持默认行为（优先DM，否则按全局角色轮询）
                dm_char_id = self.global_config.get('dm_role_id') 
                if dm_char_id and dm_char_id in self.character_files:
                    ai_char_id = dm_char_id
                else:
                    all_ai_roles = [rid for rid in self.global_config['character_roles'] if rid in self.character_files]
                    if not all_ai_roles:
                        log_warning("全局配置中没有可用的AI角色。")
                        return
                last_responder_index = self.progress.get('last_responder_index', -1)
                next_responder_index = (last_responder_index + 1) % len(all_ai_roles)
                ai_char_id = all_ai_roles[next_responder_index]
                self.progress['last_responder_index'] = next_responder_index
            
            ai_char_name = self.character_files[ai_char_id]['name']

            # --- 修复点：格式化AI角色prompt中的模板变量 ---
            ai_system_prompt = self._format_string(self.character_files[ai_char_id]['prompt'])
            messages = [{"role": "system", "content": ai_system_prompt}]
            # 添加历史
            for record in self.dialogue_history[-10:]:
                if record['type'] == 'Dialogue':
                     hist_char_id = record.get('data', {}).get('character_id')
                     role = "assistant" if hist_char_id == ai_char_id else "user"
                     content = record.get('content') or record.get('data', {}).get('content')
                     if content and hist_char_id:
                         messages.append({"role": role, "content": content})
                elif record['type'] == 'Player':
                     content = record.get('content') or record.get('data', {}).get('content')
                     if content:
                         messages.append({"role": "user", "content": content})

                # --- 【新增代码块开始】 ---
                # 将旁白作为'user'信息加入，为AI提供故事背景上下文
                elif record['type'] == 'Narration':
                     content = record.get('content') or record.get('data', {}).get('content')
                     if content:
                        messages.append({"role": "user", "content": f"（旁白：{content}）"})
                # --- 【新增代码块结束】 ---
            
            response = chat_with_deepseek(messages, ai_char_name, color_code=TermColors.CYAN)
            if response:
                self._add_to_dialogue_history('Dialogue', character_id=ai_char_id, content=response)
            
            context['turns_taken'] += 1
            if end_condition['Type'] == 'LimitedFreeTime' and context['turns_taken'] >= end_condition['MaxTurns']:
                log_info("达到最大轮次，自由时间结束。")
                self._transition_to_unit(end_condition['NextUnitID'])
                self.run()

        elif state == 'WaitingForPlayerChoice':
            context = self.progress['choice_context']
            if text in context['Branches']:
                branch = context['Branches'][text]
                self._transition_to_unit(branch['NextUnitID'])
                self.run()
            else:
                log_warning("无效的选择，请重新输入。")
                print(f"{TermColors.RED}无效选择，请输入方括号内的字母。{TermColors.RESET}")

    def _execute_ai_choice(self):
        """执行 AI 选择逻辑，包括决策和判断，然后转换剧情单元。"""
        context = self.progress.get('ai_choice_context')
        if not context:
            log_error("无法执行 AI Choice，上下文中缺少必要信息。")
            self.game_over = True
            return

        decider_id = self._format_string(context['DeciderCharacterID'])
        decider_char = self.character_files.get(decider_id)
        if not decider_char:
            log_error(f"AI Choice 失败：找不到决策角色 '{decider_id}'。")
            self.game_over = True
            return
            
        # 1. 构建决策 LLM 请求 (Decision Call)
        decision_prompt = self._format_string(context['DecisionPromptForAI'])
        decision_messages = []
        # --- 修复点：格式化决策角色prompt中的模板变量 ---
        decider_system_prompt = self._format_string(decider_char['prompt'])
        decision_messages.append({"role": "system", "content": decider_system_prompt})
        
        # 添加对话历史 (与 Dialogue Prompt 逻辑相同)
        for record in self.dialogue_history[-15:]:  # 可以适当增加历史记录长度
            # 从记录中获取纯文本内容
            content = record.get('content') or record.get('data', {}).get('content')
            if not content:
                continue

            if record['type'] == 'Dialogue':
                hist_char_id = record.get('data', {}).get('character_id')
                if hist_char_id:
                    # 根据决策者ID (decider_id) 来决定历史记录中的角色是 'assistant' 还是 'user'
                    role = "assistant" if hist_char_id == decider_id else "user"
                    decision_messages.append({"role": role, "content": content})
            elif record['type'] == 'Player':
                # 玩家的发言对决策AI来说都是 'user'
                decision_messages.append({"role": "user", "content": content})
        
        decision_messages.append({"role": "system", "content": decision_prompt})

        # 调用LLM获取决策文本 (这里不直接打印，是AI的内心思考)
        log_info(f"正在为角色 {decider_char['name']} 获取决策...")
        ai_decision_text = chat_with_deepseek(decision_messages, character_name=f"{decider_char['name']}(内心)", is_internal_thought=True, color_code=TermColors.CYAN)

        if not ai_decision_text:
            log_error("AI 未能做出决策，剧情无法继续。")
            self.game_over = True
            return
            
        log_debug(f"AI 决策原文: {ai_decision_text}")

        # 2. 构建判断 LLM 请求 (Judge Call)
        judge_prompt = self._format_string(context['JudgePromptForSystem'])
        judge_messages = [
            {"role": "system", "content": judge_prompt},
            {"role": "user", "content": f"请根据以下AI角色的决策文本进行判断：\n\n---\n{ai_decision_text}\n---"}
        ]
        
        log_info("系统正在判断 AI 的选择...")
        judged_result = chat_with_deepseek(judge_messages, character_name="系统判断", is_internal_thought=True, color_code=TermColors.CYAN)
        
        if not judged_result:
            log_error("系统未能判断 AI 的决策，剧情无法继续。")
            self.game_over = True
            return

        # 3. 处理结果并转换剧情单元
        final_choice = judged_result.strip().upper()
        log_info_color(f"AI 的选择已被系统判断为: '{final_choice}'", TermColors.GREEN)

        if final_choice in context['Branches']:
            next_unit_id = context['Branches'][final_choice]
            # 清理上下文并转换
            del self.progress['ai_choice_context']
            self._transition_to_unit(next_unit_id)
            self.run()  # 立即继续游戏循环
        else:
            log_error(f"判断结果 '{final_choice}' 无效，在 Branches 中找不到匹配项。")
            self.game_over = True