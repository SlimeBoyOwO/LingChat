import re
import random
import uuid
from datetime import datetime
from typing import Any, Dict, List, Optional

from .models import GameSession, StoryUnit
from .logger import log_debug, log_warning

class StateManager:
    """
    管理游戏会话的所有动态状态。
    包括游戏变量、进度、对话历史和运行时上下文。
    """
    def __init__(self, session: GameSession):
        self.session = session
        self.runtime_context: Dict[str, Any] = {} 
        # 用于存放不需存档的临时变量但运行需要进入上下文的，如本次用户输入 {player_input}

    @property
    def game_state(self):
        return self.session.game_state

    @property
    def progress(self):
        return self.session.game_progress

    @property
    def dialogue_history(self):
        return self.session.dialogue_history

    def format_string(self, text: str) -> str:
        """用 game_state 和 runtime_context 中的变量替换字符串中的 {placeholder}"""
        if not isinstance(text, str):
            return text
        
        placeholders = re.findall(r'\{([a-zA-Z0-9_]+)\}', text)
        
        formatted_text = text
        for placeholder in placeholders:
            value = None
            if placeholder in self.runtime_context:
                value = self.runtime_context[placeholder]
            elif placeholder in self.game_state.variables:
                value = self.game_state.get(placeholder)
            else:
                log_warning(f"格式化字符串时未找到变量: {placeholder}")
                continue

            if value is not None:
                formatted_text = formatted_text.replace(f'{{{placeholder}}}', str(value))

        return formatted_text

    def evaluate_condition(self, condition_str: str) -> bool:
        """
        安全地评估条件表达式。
        这是一个简化的、安全的实现，以替代不安全的 eval()。
        支持的格式: "变量 操作符 值"，例如: "player_hp > 10" 或 "quest_status == 'completed'"
        """
        formatted_condition = self.format_string(condition_str)
        log_debug(f"正在评估条件: `{condition_str}` -> `{formatted_condition}`")
        
        try:
            parts = formatted_condition.split()
            if len(parts) != 3:
                log_warning(f"无效的条件格式: '{formatted_condition}'")
                return False

            left_str, op, right_str = parts
            
            try:
                left = float(left_str)
                right = float(right_str)
            except ValueError:
                left = left_str.strip("'\"")
                right = right_str.strip("'\"")

            if op == '==': return left == right
            if op == '!=': return left != right
            if op == '>': return left > right
            if op == '<': return left < right
            if op == '>=': return left >= right
            if op == '<=': return left <= right
            
            log_warning(f"不支持的条件操作符: '{op}'")
            return False

        except Exception as e:
            log_warning(f"评估条件时出错: '{formatted_condition}'. 错误: {e}")
            return False

    def get_current_story_unit(self) -> Optional[StoryUnit]:
        unit_id = self.progress.pointer.current_unit_id
        return self.session.story_units.get(unit_id)

    def get_next_event(self) -> Optional[Dict[str, Any]]:
        unit = self.get_current_story_unit()
        if not unit:
            return None
        
        next_index = self.progress.pointer.last_completed_event_index + 1
        if 0 <= next_index < len(unit.events):
            return unit.events[next_index]
        return None

    def advance_event_pointer(self):
        self.progress.pointer.last_completed_event_index += 1

    def transition_to_unit(self, unit_id: str):
        log_debug(f"切换剧情单元到: {unit_id}")
        self.progress.pointer.current_unit_id = unit_id
        self.progress.pointer.last_completed_event_index = -1
        self.progress.runtime_state = 'ExecutingEvents'

    def set_variable(self, name: str, value: Any):
        log_debug(f"设置变量: {name} = {value}")
        self.game_state.set(name, value)

    def calculate_variable(self, name: str, expression: str):
        # 注意：这里仍然使用了 eval，但在一个受限的环境中，未来建议使用更安全的库，如 `asteval`。
        try:
            formatted_expr = self.format_string(expression)
            result = eval(formatted_expr, {"__builtins__": {}}, self.game_state.variables)
            self.set_variable(name, result)
        except Exception as e:
            log_warning(f"计算表达式失败: '{expression}'. 错误: {e}")

    def set_random_variable(self, name: str, min_val: Any, max_val: Any):
        try:
            i_min = int(min_val)
            i_max = int(max_val)
            
            # 使用 random.choice 从一个包含所有可能整数的列表中选择，这比 randint 更能保证分布
            possible_values = list(range(i_min, i_max + 1))
            chosen_value = random.choice(possible_values)
            self.set_variable(name, chosen_value)

            log_debug(f"随机变量 '{name}' 已从 {possible_values} 中选择值为: {chosen_value}。")
        except (ValueError, TypeError) as e:
            # 如果转换失败（例如，脚本中写了非数字），记录警告并设置一个默认值以防游戏崩溃
            log_warning(f"为 'Random' Action 提供的值无效: min='{min_val}', max='{max_val}'. 错误: {e}")
            self.set_variable(name, min_val if min_val is not None else 1)
        
    def set_random_choice_variable(self, name: str, choices: List[Any]):
        self.set_variable(name, random.choice(choices))

    def add_dialogue_history(self, event_type: str, **kwargs):
        log_entry = {
            "id": f"evt_{uuid.uuid4()}",
            "timestamp": datetime.now().isoformat(),
            "source_unit_id": self.progress.pointer.current_unit_id,
            "source_event_index": self.progress.pointer.last_completed_event_index,
            "type": event_type
        }
        
        if len(kwargs) == 1 and 'content' in kwargs:
            log_entry['content'] = kwargs['content']
        else:
            log_entry['data'] = kwargs

        self.dialogue_history.append(log_entry)
        log_debug(f"添加新对话记录: {log_entry}")