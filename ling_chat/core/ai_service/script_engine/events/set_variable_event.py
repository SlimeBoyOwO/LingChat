from typing import Any, Tuple, Optional
import re

from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.ai_service.script_engine.utils.expression import evaluate
from ling_chat.core.ai_service.script_engine.utils.script_function import ScriptFunction
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
        op, var_name, value = ScriptFunction.parse_variable_action(action_str)
        if op is None:
            logger.error(f"无法解析 action: {action_str}")
            return

        current_val = self._get_var(var_name)

        try:
            new_val = ScriptFunction.apply_variable_action(op, current_val, value)
        except Exception as e:
            logger.error(f"执行 action '{action_str}' 时出错: {e}")
            return

        self._set_var(var_name, new_val)
        logger.info(f"变量已更新: {var_name} = {new_val} (通过 {action_str})")

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'set_variable'