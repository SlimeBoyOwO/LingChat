"""
提示词配置加载器

从 ling_chat/configs/prompt.toml 加载对话格式、情绪限制、默认示例等配置
"""

from pathlib import Path
from typing import ClassVar

try:
    import tomllib  # Python 3.11+ 标准库
except ImportError:
    import tomli as tomllib  # 兼容 Python 3.10

from ling_chat.utils.runtime_path import package_root


class PromptConfig:
    """
    提示词配置单例类

    负责从 TOML 配置文件加载并提供提示词模板访问
    """

    _instance: ClassVar["PromptConfig | None"] = None
    # 配置文件路径：ling_chat/configs/prompt.toml
    _config_path: ClassVar[Path] = package_root / "configs" / "prompt.toml"

    def __new__(cls) -> "PromptConfig":
        if cls._instance is None:
            cls._instance = super().__new__(cls)
            cls._instance._load()
        return cls._instance

    def _load(self) -> None:
        """从 TOML 文件加载配置"""
        if not self._config_path.exists():
            raise FileNotFoundError(f"提示词配置文件不存在: {self._config_path}")

        with open(self._config_path, "rb") as f:
            self._config = tomllib.load(f)

    @classmethod
    def set_config_path(cls, path: str | Path) -> None:
        """设置配置文件路径（用于测试或自定义位置）"""
        cls._config_path = Path(path)

    @property
    def dialog_format_cn(self) -> str:
        """中文模式对话格式模板"""
        return self._config["dialog_format"]["cn"]["template"]

    @property
    def dialog_format_jp(self) -> str:
        """日语翻译模式对话格式模板"""
        return self._config["dialog_format"]["jp"]["template"]

    def get_emotion_limit_prompt(self, enabled: bool) -> str:
        """
        获取情绪限制提示词

        Args:
            enabled: 是否启用情绪限制（True 使用限制列表，False 使用自由模式）

        Returns:
            str: 情绪限制提示词模板
        """
        key = "enabled" if enabled else "disabled"
        return self._config["emotion_limit"][key]["template"]

    def get_default_example(self, use_cn: bool) -> str:
        """
        获取默认对话示例

        Args:
            use_cn: True 使用中文示例，False 使用日语翻译示例

        Returns:
            str: 默认对话示例文本
        """
        key = "cn" if use_cn else "jp"
        return self._config["default_examples"][key]["text"]

    @property
    def format_prompt_template(self) -> str:
        """获取对话格式提示模板"""
        return self._config["format_prompt_template"]["template"]

    def reload(self) -> None:
        """重新加载配置文件"""
        self._load()


# 全局配置实例
prompt_config = PromptConfig()
