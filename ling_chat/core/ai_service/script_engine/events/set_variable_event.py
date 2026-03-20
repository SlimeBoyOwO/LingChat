from typing import Any, Tuple, Optional
import re

from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.ai_service.script_engine.utils.expression import evaluate
from ling_chat.core.logger import logger


class SetVariableEvent(BaseEvent):
    """处理变量设置事件"""

    async def execute(self):
        options = self.event_data.get('options', [])
        if not options:
            logger.warning("SetVariableEvent 没有提供 options")
            return

        for opt in options:
            condition = opt.get('condition')
            action = opt.get('action')

            if not action:
                logger.warning("SetVariableEvent 选项缺少 action，跳过")
                continue

            # 判断条件是否满足
            condition_met = True
            if condition:
                vars_dict = self._get_vars_dict()
                condition_met = evaluate(condition, vars_dict)

            if condition_met:
                # 执行 action
                self._execute_action(action)
                return  # 只执行第一个满足的 action

    def _execute_action(self, action_str: str):
        op, var_name, value = self._parse_action(action_str)
        if op is None:
            logger.error(f"无法解析 action: {action_str}")
            return

        current_val = self._get_var(var_name)

        try:
            if op == 'assign':
                new_val = value
            elif op == 'add':
                new_val = current_val + value if current_val is not None else value
            elif op == 'sub':
                new_val = current_val - value if current_val is not None else -value
            else:
                logger.error(f"未知的操作符: {op}")
                return
        except TypeError as e:
            logger.error(f"执行 action '{action_str}' 时类型错误: {e}")
            return

        self._set_var(var_name, new_val)
        logger.info(f"变量已更新: {var_name} = {new_val} (通过 {action_str})")

    def _parse_action(self, action: str) -> Tuple[Optional[str], Optional[str], Any]:
        action = action.strip()
        match = re.match(r'^([a-zA-Z_][a-zA-Z0-9_]*)\s*([+\-]?=)\s*(.+)$', action)
        if not match:
            return None, None, None

        var_name = match.group(1)
        operator = match.group(2)
        value_str = match.group(3).strip()
        value = self._parse_value(value_str)

        if operator == '=':
            return 'assign', var_name, value
        elif operator == '+=':
            return 'add', var_name, value
        elif operator == '-=':
            return 'sub', var_name, value
        else:
            return None, None, None

    def _parse_value(self, s: str) -> Any:
        s_lower = s.lower()
        if s_lower == 'true':
            return True
        if s_lower == 'false':
            return False

        try:
            if '.' in s:
                return float(s)
            else:
                return int(s)
        except ValueError:
            pass

        if (s.startswith('"') and s.endswith('"')) or (s.startswith("'") and s.endswith("'")):
            return s[1:-1]

        return s

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'set_variable'