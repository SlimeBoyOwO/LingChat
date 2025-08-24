# neochat/game/end_conditions/__init__.py
import os
import importlib
import inspect
from .base import BaseEndConditionHandler

# 全局结局条件处理器注册表
end_condition_registry = {}

# 自动发现并加载当前目录下的所有处理器
current_dir = os.path.dirname(__file__)
for filename in os.listdir(current_dir):
    if filename.endswith('.py') and not filename.startswith(('__', 'base.')):
        module_name = filename[:-3]
        module = importlib.import_module(f'.{module_name}', package=__name__)

        for name, obj in inspect.getmembers(module, inspect.isclass):
            if issubclass(obj, BaseEndConditionHandler) and obj is not BaseEndConditionHandler:
                # 将类名的大写驼峰转为结局类型名
                # e.g., "BranchingEndConditionHandler" -> "Branching"
                end_type_name = name.replace("EndConditionHandler", "")

                # 特殊处理：FreeTime 处理器也负责 LimitedFreeTime
                if end_type_name == "FreeTime":
                    end_condition_registry["FreeTime"] = obj
                    end_condition_registry["LimitedFreeTime"] = obj
                else:
                    end_condition_registry[end_type_name] = obj
                # print(f"[EndConditionSystem] Registered handler for '{end_type_name}': {obj.__name__}")