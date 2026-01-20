import importlib
import inspect
from pathlib import Path
from typing import Dict, Optional, Type

from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.logger import logger


class EventHandlerLoader:
    _event_handlers: Dict[str, Type[BaseEvent]] = {}
    _loaded = False

    @classmethod
    def _ensure_loaded(cls):
        """确保事件处理器已加载"""
        if cls._loaded:
            return

        events_dir = "ling_chat/core/ai_service/script_engine/events"
        events_path = Path(events_dir)

        if not events_path.exists():
            logger.warning(f"事件目录不存在: {events_dir}")
            cls._loaded = True
            return

        # 遍历事件目录中的所有Python文件
        for file_path in events_path.glob("*.py"):
            if file_path.name == "__init__.py":
                continue

            module_name = f"ling_chat.core.ai_service.script_engine.events.{file_path.stem}"
            try:
                module = importlib.import_module(module_name)

                # 查找模块中的所有类
                for name, obj in inspect.getmembers(module, inspect.isclass):
                    if (issubclass(obj, BaseEvent) and
                        obj != BaseEvent and
                        hasattr(obj, 'can_handle')):

                        # 注册事件处理器
                        cls._event_handlers[obj.__name__] = obj
                        logger.debug(f"加载事件处理器: {obj.__name__}")

            except ImportError as e:
                logger.error(f"加载模块 {module_name} 失败: {e}")

        cls._loaded = True

    @classmethod
    def get_handler_for_event(cls, event_data: dict) -> Optional[Type[BaseEvent]]:
        """获取适合处理指定事件的处理程序类"""
        cls._ensure_loaded()

        event_type = event_data.get('type', '')

        for handler_class in cls._event_handlers.values():
            if handler_class.can_handle(event_type):
                return handler_class

        logger.error(f"没有找到适合处理事件类型 '{event_type}' 的处理器")
        return None

    @classmethod
    def create_event_instance(cls, event_data: dict, game_context) -> Optional[BaseEvent]:
        """创建事件实例"""
        handler_class = cls.get_handler_for_event(event_data)
        if handler_class:
            return handler_class(event_data, game_context)
        return None
