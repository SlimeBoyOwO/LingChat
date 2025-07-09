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
    'spark': 'spark_adapter',
    'qwen': 'qwen_adapter'
}

def get_adapter(model_name):
    for prefix, adapter in MODEL_MAPPING.items():
        if model_name.startswith(prefix):
            module = importlib.import_module(f'adapters.{adapter}')
            return module.Adapter(CONFIG[adapter.removesuffix('_adapter')])
    raise ValueError(f"Unsupported model: {model_name}")