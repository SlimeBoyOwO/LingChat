# neochat/llm/__init__.py

# 这一行代码使得项目的其他部分可以通过 `from neochat.llm import llm_manager`
# 来直接访问 LLMManager 的单例，简化调用。
from .manager import llm_manager

__all__ = ["llm_manager"]