# core/events/__init__.py
import os
import importlib
import inspect
from .base import BaseEventHandler

# 全局事件处理器注册表
event_registry = {}

def register_event_handler(event_type: str):
    def decorator(cls):
        if issubclass(cls, BaseEventHandler):
            event_registry[event_type] = cls
        return cls
    return decorator

# 自动发现并加载当前目录下的所有事件处理器
current_dir = os.path.dirname(__file__)
for filename in os.listdir(current_dir):
    if filename.endswith('.py') and not filename.startswith(('__', 'base.')):
        module_name = filename[:-3]
        module = importlib.import_module(f'.{module_name}', package=__name__)
        
        # 自动将类名的大写驼峰转为事件类型（如 DialogueEventHandler -> Dialogue）
        for name, obj in inspect.getmembers(module, inspect.isclass):
            if issubclass(obj, BaseEventHandler) and obj is not BaseEventHandler:
                # "DialogueEventHandler" -> "Dialogue"
                event_type_name = name.replace("EventHandler", "")
                event_registry[event_type_name] = obj
                print(f"[EventSystem] Registered handler: {event_type_name} -> {obj.__name__}")