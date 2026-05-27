"""LLM配置管理器，支持多配置方案存储和切换

底层逻辑：将LLM配置从.env分离到独立的TOML文件夹，实现多配置方案切换
"""

import os
import threading
import tomllib
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional

from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import package_root


class LLMConfig:
    """LLM配置管理器单例类

    支持多配置方案存储、热切换、从.env自动迁移
    """

    _instance: Optional["LLMConfig"] = None
    _lock = threading.Lock()
    _initialized = False

    def __new__(cls) -> "LLMConfig":
        if cls._instance is None:
            with cls._lock:
                if cls._instance is None:
                    cls._instance = super().__new__(cls)
        return cls._instance

    def __init__(self) -> None:
        if self._initialized:
            return

        self._initialized = True
        self._config_dir: Path = package_root / "configs" / "llm_configs"
        self._active_config_name: str = "default"
        self._config: Dict[str, Any] = {}
        self._callbacks: List[Callable[[], None]] = []

        self._init_config_dir()
        self._load_active()

    def _init_config_dir(self) -> None:
        """初始化配置文件夹，首次启动时从.env迁移"""
        self._config_dir.mkdir(parents=True, exist_ok=True)

        # 检查是否存在任何toml配置文件
        existing_tomls = list(self._config_dir.glob("*.toml"))
        if not existing_tomls:
            logger.info("检测到首次启动，从.env迁移LLM配置...")
            self._migrate_from_env()

    def _migrate_from_env(self) -> None:
        """从.env提取LLM配置并生成default.toml"""
        # 提取LLM相关环境变量
        llm_config = self._extract_env_llm_config()

        # 写入default.toml
        default_path = self._config_dir / "default.toml"
        self._write_toml(default_path, llm_config)
        logger.info(f"已创建默认LLM配置: {default_path}")

    def _extract_env_llm_config(self) -> Dict[str, Any]:
        """从环境变量提取LLM配置（兼容旧provider专用环境变量）"""
        provider = os.environ.get("LLM_PROVIDER", "webllm")

        # 通用字段
        main = {
            "provider": provider,
            "temperature": float(os.environ.get("TEMPERATURE", "1.3")),
            "top_p": float(os.environ.get("TOP_P", "0.9")),
            "enable_thinking": os.environ.get("ENABLE_THINKING", "none").lower(),
            "proxy": "",
        }

        # 按 provider 映射旧环境变量到统一字段
        provider_mappings = {
            "webllm": {
                "api_key": ("CHAT_API_KEY", ""),
                "base_url": ("CHAT_BASE_URL", "https://api.deepseek.com/v1"),
                "model": ("MODEL_TYPE", "deepseek-chat"),
                "proxy": ("CHAT_PROXY_URL", ""),
            },
            "gemini": {
                "api_key": ("GEMINI_API_KEY", ""),
                "base_url": ("GEMINI_BASE_URL", "https://generativelanguage.googleapis.com/v1beta"),
                "model": ("GEMINI_MODEL_TYPE", "gemini-2.5-flash"),
                "proxy": ("GEMINI_PROXY_URL", ""),
            },
            "ollama": {
                "base_url": ("OLLAMA_BASE_URL", "http://localhost:11434"),
                "model": ("OLLAMA_MODEL", "llama3"),
            },
            "lmstudio": {
                "api_key": ("LMSTUDIO_API_KEY", ""),
                "base_url": ("LMSTUDIO_BASE_URL", "http://localhost:1234/"),
                "model": ("LMSTUDIO_MODEL_TYPE", "unknown"),
            },
        }

        mapping = provider_mappings.get(provider, provider_mappings["webllm"])
        for key, (env_var, default) in mapping.items():
            main[key] = os.environ.get(env_var, default)

        return {
            "config_name": "默认配置",
            "config_description": "从.env自动迁移的默认配置",
            "main": main,
            "translator": {
                "provider": os.environ.get("TRANSLATE_LLM_PROVIDER", "none"),
                "model": os.environ.get("TRANSLATE_MODEL", ""),
                "api_key": os.environ.get("TRANSLATE_API_KEY", ""),
                "base_url": os.environ.get("TRANSLATE_BASE_URL", ""),
            },
            "providers": {},
        }

    def _load_active(self) -> None:
        """加载当前激活的配置"""
        config_path = self._config_dir / f"{self._active_config_name}.toml"
        if not config_path.exists():
            # 如果激活的配置不存在，回退到default
            config_path = self._config_dir / "default.toml"
            self._active_config_name = "default"
            if not config_path.exists():
                self._config = self._create_default_config()
                return

        self._config = self._parse_toml(config_path)

    def _parse_toml(self, path: Path) -> Dict[str, Any]:
        """解析TOML文件"""
        try:
            with open(path, "rb") as f:
                return tomllib.load(f)
        except Exception as e:
            logger.error(f"解析TOML文件失败 {path}: {e}")
            return self._create_default_config()

    def _write_toml(self, path: Path, config: Dict[str, Any]) -> None:
        """写入TOML文件（手动序列化以支持Python 3.11+）"""
        try:
            lines = []

            # 添加元数据注释
            name = config.get("config_name", "未命名配置")
            desc = config.get("config_description", "")
            lines.append(f'# config_name = "{name}"')
            if desc:
                lines.append(f'# config_description = "{desc}"')
            lines.append("")

            # 写入main配置
            if "main" in config:
                lines.append("[main]")
                for key, value in config["main"].items():
                    lines.append(self._format_toml_line(key, value))
                lines.append("")

            # 写入translator配置
            if "translator" in config:
                lines.append("[translator]")
                for key, value in config["translator"].items():
                    lines.append(self._format_toml_line(key, value))
                lines.append("")

            # 写入providers配置
            if "providers" in config:
                for provider, pconfig in config["providers"].items():
                    if pconfig:
                        lines.append(f"[providers.{provider}]")
                        for key, value in pconfig.items():
                            lines.append(self._format_toml_line(key, value))
                        lines.append("")

            with open(path, "w", encoding="utf-8") as f:
                f.write("\n".join(lines))

            logger.debug(f"已写入TOML配置: {path}")
        except Exception as e:
            logger.error(f"写入TOML文件失败 {path}: {e}")
            raise

    def _format_toml_line(self, key: str, value: Any) -> str:
        """格式化TOML行"""
        if isinstance(value, str):
            if "\"" in value:
                return f'{key} = \'{value}\''
            return f'{key} = "{value}"'
        elif isinstance(value, bool):
            return f"{key} = {str(value).lower()}"
        elif isinstance(value, (int, float)):
            return f"{key} = {value}"
        return f'{key} = "{value}"'

    def _create_default_config(self) -> Dict[str, Any]:
        """创建默认配置"""
        return {
            "config_name": "默认配置",
            "config_description": "自动生成的默认配置",
            "main": {
                "provider": "webllm",
                "model": "deepseek-chat",
                "api_key": "",
                "base_url": "https://api.deepseek.com/v1",
                "proxy": "",
                "temperature": 1.3,
                "top_p": 0.9,
                "enable_thinking": "none",
            },
            "translator": {
                "provider": "none",
                "model": "",
                "api_key": "",
                "base_url": "",
            },
            "providers": {},
        }

    def register_reload_callback(self, callback: Callable[[], None]) -> None:
        """注册配置重载回调"""
        self._callbacks.append(callback)

    def _notify_reload(self) -> None:
        """通知所有注册的回调"""
        for callback in self._callbacks:
            try:
                callback()
            except Exception as e:
                logger.error(f"配置重载回调执行失败: {e}")

    # ============ 公开API ============

    def get_active_config_name(self) -> str:
        """获取当前激活的配置名称"""
        return self._active_config_name

    def set_active_config(self, name: str) -> bool:
        """切换激活配置

        Args:
            name: 配置方案名称

        Returns:
            是否切换成功
        """
        config_path = self._config_dir / f"{name}.toml"
        if not config_path.exists():
            logger.error(f"配置方案不存在: {name}")
            return False

        self._active_config_name = name
        self._load_active()
        self._notify_reload()
        logger.info(f"已切换LLM配置方案: {name}")
        return True

    def get_active_config(self) -> Dict[str, Any]:
        """获取当前激活的完整配置"""
        return self._config.copy()

    def get_main_config(self) -> Dict[str, Any]:
        """获取主对话模型配置"""
        return self._config.get("main", self._create_default_config()["main"])

    def get_translator_config(self) -> Dict[str, Any]:
        """获取翻译模型配置

        如果translator.provider为none或空，返回main配置
        """
        trans = self._config.get("translator", {})
        if trans.get("provider", "none") in ["none", ""]:
            return self.get_main_config()
        return trans

    def get_provider_config(self, provider: str) -> Dict[str, Any]:
        """获取指定提供商的配置"""
        providers = self._config.get("providers", {})
        return providers.get(provider, {})

    def list_configs(self) -> List[Dict[str, Any]]:
        """列出所有可用配置方案"""
        configs = []
        for f in sorted(self._config_dir.glob("*.toml")):
            try:
                cfg = self._parse_toml(f)
                configs.append({
                    "name": f.stem,
                    "display_name": cfg.get("config_name", f.stem),
                    "description": cfg.get("config_description", ""),
                    "is_active": f.stem == self._active_config_name,
                })
            except Exception as e:
                logger.warning(f"跳过损坏的配置文件 {f}: {e}")
        return configs

    def save_config(self, name: str, config: Dict[str, Any]) -> None:
        """保存/更新配置方案

        Args:
            name: 配置方案名称
            config: 配置字典
        """
        path = self._config_dir / f"{name}.toml"
        self._write_toml(path, config)

        if name == self._active_config_name:
            self._config = config.copy()
            self._notify_reload()

        logger.info(f"已保存LLM配置方案: {name}")

    def delete_config(self, name: str) -> None:
        """删除配置方案

        Args:
            name: 配置方案名称（不允许删除default）

        Raises:
            ValueError: 尝试删除default配置
        """
        if name == "default":
            raise ValueError("default配置不可删除")

        path = self._config_dir / f"{name}.toml"
        if path.exists():
            path.unlink()
            logger.info(f"已删除LLM配置方案: {name}")

        # 如果删除的是当前激活配置，切换回default
        if name == self._active_config_name:
            self.set_active_config("default")

    def reload(self) -> None:
        """热重载配置"""
        self._load_active()
        self._notify_reload()
        logger.info(f"已重载LLM配置: {self._active_config_name}")


# 单例实例
llm_config: LLMConfig = LLMConfig()
