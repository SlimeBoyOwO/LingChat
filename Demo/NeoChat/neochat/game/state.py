# neochat/game/state.py
import re
import random
import uuid
from datetime import datetime
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional

# 导入新的日志模块
from neochat.platform.logging import log_debug, log_warning

# ==============================================================================
# 1. 数据模型 (原 models.py)
#    这些 dataclass 定义了游戏会话中所有数据的结构。
# ==============================================================================

@dataclass
class GameState:
    """存储游戏世界中的所有变量，如玩家姓名、NPC好感度等。"""
    variables: Dict[str, Any] = field(default_factory=dict)

    def get(self, key: str, default: Any = None) -> Any:
        return self.variables.get(key, default)

    def set(self, key: str, value: Any):
        self.variables[key] = value

@dataclass
class ProgressPointer:
    """指向当前剧情单元和事件的指针。"""
    current_unit_id: str
    last_completed_event_index: int = -1

@dataclass
class GameProgress:
    """存储游戏的元数据和进度。"""
    save_name: str
    story_pack_id: str
    last_saved_timestamp: str
    pointer: ProgressPointer
    runtime_state: str = "ExecutingEvents"
    context: Dict[str, Any] = field(default_factory=dict)

@dataclass
class Character:
    """代表一个AI角色。"""
    role_id: str
    name: str
    prompt: str

@dataclass
class Player:
    """代表玩家角色。"""
    name: str
    prompt: Optional[str] = ""

@dataclass
class StoryUnit:
    """代表一个剧情单元文件 (.yaml) 的内容。"""
    unit_id: str
    events: List[Dict[str, Any]]
    end_condition: Optional[Dict[str, Any]]

@dataclass
class StoryPack:
    """代表一个完整的剧本包。"""
    pack_id: str
    start_unit_id: str
    character_roles: List[str]
    dm_role_id: Optional[str] = None

@dataclass
class GameSession:
    """一个总的容器，包含一次游戏会话的所有数据。"""
    save_path: str
    game_state: GameState
    game_progress: GameProgress
    dialogue_history: List[Dict[str, Any]]
    story_pack_config: StoryPack
    story_units: Dict[str, StoryUnit]
    characters: Dict[str, Character]
    player: Player

# ==============================================================================
# 2. 状态管理器 (原 state_manager.py)
#    这个类封装了对 GameSession 中所有动态状态的操作。
# ==============================================================================

class StateManager:
    """
    管理游戏会话的所有动态状态。
    包括游戏变量、进度、对话历史和运行时上下文。
    """
    def __init__(self, session: GameSession):
        self.session = session
        # 用于存放不需存档的临时变量，如本次用户输入 {player_input}
        self.runtime_context: Dict[str, Any] = {}

    @property
    def game_state(self) -> GameState:
        return self.session.game_state

    @property
    def progress(self) -> GameProgress:
        return self.session.game_progress

    @property
    def dialogue_history(self) -> List[Dict[str, Any]]:
        return self.session.dialogue_history

    def format_string(self, text: Any) -> Any:
        """用 game_state 和 runtime_context 中的变量替换字符串中的 {placeholder}"""
        if not isinstance(text, str):
            return text

        # 使用正则表达式一次性找到所有占位符
        placeholders = re.findall(r'\{([a-zA-Z0-9_]+)\}', text)
        if not placeholders:
            return text

        formatted_text = text
        for placeholder in set(placeholders): # 使用 set 去重
            value = None
            if placeholder in self.runtime_context:
                value = self.runtime_context[placeholder]
            elif placeholder in self.game_state.variables:
                value = self.game_state.get(placeholder)
            else:
                log_warning(f"格式化字符串时未找到变量: {{{placeholder}}}")
                continue

            if value is not None:
                formatted_text = formatted_text.replace(f'{{{placeholder}}}', str(value))

        return formatted_text

    def evaluate_condition(self, condition_str: str) -> bool:
        """
        安全地评估条件表达式。
        支持格式: "变量 操作符 值"，例如: "player_hp > 10" 或 "quest_status == 'completed'"
        """
        formatted_condition = self.format_string(condition_str)
        log_debug(f"正在评估条件: `{condition_str}` -> `{formatted_condition}`")

        try:
            # 使用更健壮的正则表达式来解析
            match = re.match(r'^\s*(\S+)\s*(==|!=|>|<|>=|<=)\s*(\S+)\s*$', formatted_condition)
            if not match:
                log_warning(f"无效的条件格式: '{formatted_condition}'")
                return False

            left_str, op, right_str = match.groups()

            # 尝试将操作数转为数字，如果失败则作为字符串处理
            try:
                left = float(left_str)
                right = float(right_str)
            except ValueError:
                # 移除字符串两端的引号
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
        """获取当前正在执行的剧情单元。"""
        unit_id = self.progress.pointer.current_unit_id
        return self.session.story_units.get(unit_id)

    def get_next_event(self) -> Optional[Dict[str, Any]]:
        """获取当前剧情单元中下一个待执行的事件。"""
        unit = self.get_current_story_unit()
        if not unit:
            return None

        next_index = self.progress.pointer.last_completed_event_index + 1
        if 0 <= next_index < len(unit.events):
            return unit.events[next_index]
        return None

    def advance_event_pointer(self):
        """将事件指针向前移动一位。"""
        self.progress.pointer.last_completed_event_index += 1

    def transition_to_unit(self, unit_id: str):
        """切换到新的剧情单元。"""
        log_debug(f"切换剧情单元到: {unit_id}")
        self.progress.pointer.current_unit_id = unit_id
        self.progress.pointer.last_completed_event_index = -1
        self.progress.runtime_state = 'ExecutingEvents'
        self.runtime_context.clear() # 切换单元时清空临时上下文

    def set_variable(self, name: str, value: Any):
        """设置一个游戏状态变量。"""
        log_debug(f"设置变量: {name} = {value}")
        self.game_state.set(name, self.format_string(value))

    def calculate_variable(self, name: str, expression: str):
        """通过计算表达式来设置一个游戏状态变量。"""
        # 警告：eval() 有安全风险。在生产环境中，建议替换为更安全的库，如 asteval。
        try:
            formatted_expr = self.format_string(expression)
            # 构建一个安全的上下文环境
            safe_globals = {"__builtins__": {}}
            safe_locals = self.game_state.variables.copy()
            result = eval(formatted_expr, safe_globals, safe_locals)
            self.set_variable(name, result)
        except Exception as e:
            log_warning(f"计算表达式失败: '{expression}'. 错误: {e}")

    def set_random_variable(self, name: str, min_val: Any, max_val: Any):
        """设置一个在指定范围内的随机整数变量。"""
        try:
            i_min = int(self.format_string(min_val))
            i_max = int(self.format_string(max_val))
            chosen_value = random.randint(i_min, i_max)
            self.set_variable(name, chosen_value)
            log_debug(f"随机变量 '{name}' 已设置为: {chosen_value} (范围: [{i_min}, {i_max}])")
        except (ValueError, TypeError) as e:
            log_warning(f"为 'Random' Action 提供的值无效: min='{min_val}', max='{max_val}'. 错误: {e}")
            self.set_variable(name, min_val) # 失败时使用最小值作为回退

    def set_random_choice_variable(self, name: str, choices: List[Any]):
        """从列表中随机选择一个值来设置变量。"""
        if choices and isinstance(choices, list):
            chosen_value = random.choice(choices)
            self.set_variable(name, chosen_value)
        else:
            log_warning(f"为 'RandomChoice' 提供的 choices 无效: {choices}")

    def add_dialogue_history(self, event_type: str, **kwargs):
        """向对话历史记录中添加一条新条目。"""
        log_entry = {
            "id": f"evt_{uuid.uuid4()}",
            "timestamp": datetime.now().isoformat(),
            "source_unit_id": self.progress.pointer.current_unit_id,
            "source_event_index": self.progress.pointer.last_completed_event_index,
            "type": event_type
        }

        # 为了简化历史记录结构，将单 'content' 提升到顶层
        if len(kwargs) == 1 and 'content' in kwargs:
            log_entry['content'] = kwargs['content']
        else:
            log_entry['data'] = kwargs

        self.dialogue_history.append(log_entry)
        log_debug(f"添加新对话记录: {log_entry}")