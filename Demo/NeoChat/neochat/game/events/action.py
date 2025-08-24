# neochat/game/events/action.py
from typing import Dict, Any
from .base import BaseEventHandler
# 更新日志模块的导入路径
from neochat.platform.logging import log_error

class ActionEventHandler(BaseEventHandler):
    """
    处理游戏中的各种“动作”事件，主要用于修改游戏状态变量。
    支持 Set (设置), Calculate (计算), Random (随机数), RandomChoice (随机选择)。
    """
    def handle(self, params: Dict, content: Dict) -> bool:
        tool = params.get('Tool')
        var_name = params.get('Variable')

        if not tool or not var_name:
            log_error(f"Action 事件缺少 'Tool' 或 'Variable' 参数: {params}")
            return True # 不阻塞游戏流程

        if tool == 'Set':
            # 设置一个变量，值可能需要格式化
            self.state.set_variable(var_name, content.get('Value'))
        elif tool == 'Calculate':
            # 计算一个表达式并设置变量
            self.state.calculate_variable(var_name, content.get('Expression'))
        elif tool == 'Random':
            # 设置一个在指定范围内的随机整数
            self.state.set_random_variable(var_name, params.get('Min'), params.get('Max'))
        elif tool == 'RandomChoice':
            # 从给定列表中随机选择一个值设置变量
            self.state.set_random_choice_variable(var_name, content.get('Choices'))
        else:
            log_error(f"未知的 Action Tool: {tool}")
        return True # Action 事件通常不阻塞游戏流程