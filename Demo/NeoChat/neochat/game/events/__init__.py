# neochat/game/events/__init__.py
import os
import importlib
import inspect
from .base import BaseEventHandler

# 全局事件处理器注册表
event_registry = {}

# 自动发现并加载当前目录下的所有事件处理器
current_dir = os.path.dirname(__file__)
for filename in os.listdir(current_dir):
    # 确保是Python文件，且不是 __init__.py 或 base.py
    if filename.endswith('.py') and not filename.startswith(('__', 'base.')):
        module_name = filename[:-3]
        # 动态导入模块
        module = importlib.import_module(f'.{module_name}', package=__name__)

        # 遍历模块中的所有类
        for name, obj in inspect.getmembers(module, inspect.isclass):
            # 确保类是 BaseEventHandler 的子类，但不是 BaseEventHandler 本身
            if issubclass(obj, BaseEventHandler) and obj is not BaseEventHandler:
                # 自动将类名的大写驼峰转为事件类型名
                # 例如: "DialogueEventHandler" -> "Dialogue"
                event_type_name = name.replace("EventHandler", "")
                event_registry[event_type_name] = obj
                # 可以在启动时打印日志来确认注册情况
                # print(f"[EventSystem] Registered handler: {event_type_name} -> {obj.__name__}")