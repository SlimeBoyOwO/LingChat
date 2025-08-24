# neochat/platform/configuration.py
import os
import yaml
from dotenv import load_dotenv
from typing import Dict, Any

class Config:
    """
    一个用于加载和访问应用配置的类。
    它将 YAML 文件中的配置项作为类的属性，方便访问。
    """
    def __init__(self, config_path="config.yaml"):
        # 加载 .env 文件中的环境变量
        load_dotenv()

        # 读取 YAML 配置文件
        with open(config_path, 'r', encoding='utf-8') as f:
            self._raw_config = yaml.safe_load(f)

        # 递归地将字典转换为类的属性
        self._load_config_recursively(self._raw_config, self)

    def _load_config_recursively(self, config_dict: Dict[str, Any], target_obj: Any):
        """
        递归地将字典的键值对设置成对象的属性。
        如果值是字典，会创建一个新的内部类实例来承载。
        """
        for key, value in config_dict.items():
            # 替换环境变量占位符，例如 ${API_KEY}
            if isinstance(value, str) and value.startswith('${') and value.endswith('}'):
                env_var_name = value[2:-1]
                value = os.getenv(env_var_name)

            if isinstance(value, dict):
                # 为嵌套的字典创建一个新的内部对象
                nested_obj = type(key, (object,), {})()
                setattr(target_obj, key, nested_obj)
                self._load_config_recursively(value, nested_obj)
            else:
                setattr(target_obj, key, value)

# 创建一个全局配置实例，供整个应用程序导入和使用。
# 这样可以确保配置只被加载一次。
config = Config()

# 使用示例 (在其他文件中):
# from neochat.platform.configuration import config
# print(config.llm.api_url)
# print(config.paths.saves)