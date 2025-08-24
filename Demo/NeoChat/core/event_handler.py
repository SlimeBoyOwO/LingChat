# core/event_handler.py
from typing import Any, Dict
from .state_manager import StateManager
from .ui import ConsoleUI
from .logger import log_debug, log_error
from .events import event_registry # 导入动态注册表

class EventHandler:
    """
    一个分发器，根据事件类型从注册表中查找并调用相应的处理器。
    """
    def __init__(self, state_manager: StateManager, ui: ConsoleUI):
        self.state = state_manager
        self.ui = ui
        # 缓存处理器实例以提高性能
        self.handler_instances = {}

    def handle_event(self, event_data: Dict[str, Any]) -> bool:
        """事件处理的入口点。"""
        log_debug(f"处理事件: {event_data}")

        # 条件检查逻辑保持不变
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

        handler_class = event_registry.get(event_type)
        if not handler_class:
            log_error(f"未找到事件类型 '{event_type}' 的处理器。")
            return True

        if event_type not in self.handler_instances:
            self.handler_instances[event_type] = handler_class(self.state, self.ui)
        
        handler = self.handler_instances[event_type]

        # 格式化内容
        if isinstance(event_content, str):
            content = self.state.format_string(event_content)
        elif isinstance(event_content, dict):
            content = {k: self.state.format_string(v) for k, v in event_content.items()}
        else:
            content = event_content

        return handler.handle(params, content)