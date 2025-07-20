# adapters/__init__.py
import os
import importlib
import yaml

current_dir = os.path.dirname(os.path.abspath(__file__))
config_path = os.path.join(current_dir, '..', 'config.yaml')
with open(config_path, encoding='utf-8') as f:
    CONFIG = yaml.safe_load(f)

MODEL_MAPPING = {
    'gpt-': 'openai_adapter',
    'deepseek-': 'deepseek_adapter',
    'ollama-': 'ollama_adapter',
}

def get_adapter(model_name):
    for prefix, adapter in MODEL_MAPPING.items():
        if model_name.startswith(prefix):
            module = importlib.import_module(f'adapters.{adapter}')
            config_key = adapter.removesuffix('_adapter')
            if config_key not in CONFIG:
                raise ValueError(f"Missing configuration for {config_key}")
            return module.Adapter(CONFIG[config_key])
    raise ValueError(f"Unsupported model: {model_name}")