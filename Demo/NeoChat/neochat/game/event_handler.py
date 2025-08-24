# neochat/game/event_handler.py
from typing import Any, Dict

# 更新 import
from neochat.game.state import StateManager
from neochat.presentation.cli.ui import ConsoleUI
from neochat.platform.logging import log_debug, log_error
from neochat.game.events import event_registry # 从新的注册表导入

class EventHandler:
    """
    一个分发器，根据事件类型从注册表中查找并调用相应的处理器。
    """
    def __init__(self, state_manager: StateManager, ui: ConsoleUI):
        self.state = state_manager
        self.ui = ui
        # 缓存处理器实例以提高性能
        self.handler_instances: Dict[str, Any] = {}

    def handle_event(self, event_data: Dict[str, Any]) -> bool:
        """事件处理的入口点。"""
        log_debug(f"处理事件: {event_data}")

        # 条件检查逻辑
        if 'Condition' in event_data:
            if not self.state.evaluate_condition(event_data['Condition']):
                log_debug("条件不满足，跳过事件块。")
                return True
            # 如果条件满足，则执行嵌套事件
            for nested_event in event_data.get('Events', []):
                if not self.handle_event(nested_event):
                    return False # 如果任何一个嵌套事件需要暂停，则整个块暂停
            return True

        if not event_data:
            log_error("接收到空的事件数据。")
            return True

        event_key, event_content = list(event_data.items())[0]
        # 保持此处的解析逻辑，它与你的YAML剧本文件格式一致
        # 预期格式：'Type: Dialogue | Character: Yuki'
        params = dict(param.strip().split(': ', 1) for param in event_key.split(' | '))
        event_type = params.get('Type')

        if not event_type:
            log_error(f"事件缺少 'Type' 参数: {event_key}")
            return True

        # 从注册表查找处理器类
        handler_class = event_registry.get(event_type)
        if not handler_class:
            log_error(f"未找到事件类型 '{event_type}' 的处理器。")
            return True

        # 实例化并缓存处理器
        if event_type not in self.handler_instances:
            self.handler_instances[event_type] = handler_class(self.state, self.ui)
        handler = self.handler_instances[event_type]

        # 格式化内容中的变量
        if isinstance(event_content, str):
            content = self.state.format_string(event_content)
        elif isinstance(event_content, dict):
            # 递归地格式化字典中的所有字符串值
            content = {k: self.state.format_string(v) for k, v in event_content.items()}
        else:
            content = event_content

        # 调用处理器的 handle 方法
        return handler.handle(params, content)