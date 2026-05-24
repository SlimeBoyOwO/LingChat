import os
import threading
from typing import Dict

from ling_chat.configs.llm_config import llm_config
from ling_chat.core.service_manager import service_manager

_runtime_update_lock = threading.Lock()

# LLM相关环境变量key（用于触发热重载）
LLM_CONFIG_KEYS = {
    "LLM_PROVIDER", "MODEL_TYPE", "CHAT_API_KEY", "CHAT_BASE_URL",
    "TEMPERATURE", "TOP_P", "ENABLE_THINKING",
    "TRANSLATE_LLM_PROVIDER", "TRANSLATE_MODEL", "TRANSLATE_API_KEY", "TRANSLATE_BASE_URL",
    "OLLAMA_BASE_URL", "OLLAMA_MODEL",
    "LMSTUDIO_BASE_URL", "LMSTUDIO_MODEL_TYPE", "LMSTUDIO_API_KEY",
    "GEMINI_API_KEY", "GEMINI_MODEL_TYPE", "GEMINI_PROXY_URL",
}


def apply_runtime_config_changes(new_values: Dict[str, str]) -> None:
    """应用运行时配置更改，支持热重载"""
    with _runtime_update_lock:
        # 检测是否有LLM配置变更
        has_llm_changes = any(key in LLM_CONFIG_KEYS for key in new_values.keys())

        # 更新环境变量（保持向后兼容）
        for key, value in new_values.items():
            if key in LLM_CONFIG_KEYS:
                # LLM配置不写入环境变量，由LLMConfig直接管理
                continue
            os.environ[str(key)] = str(value)

        # 如果有LLM配置变更，触发LLMConfig重载
        if has_llm_changes:
            llm_config.reload()

        # 通知AIService应用配置更改
        ai_service = service_manager.ai_service
        if ai_service is not None:
            ai_service.apply_runtime_config(new_values)
