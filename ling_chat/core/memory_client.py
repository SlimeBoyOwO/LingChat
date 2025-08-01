import sys
import os

# 添加项目根目录到 Python 路径
project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, project_root)

from ling_chat.core.memory_rag.graph import MemoryGraph, get_default_graph_config
from ling_chat.core.memory_rag.vector import MemoryVector, get_default_vector_config

_memory_client = None

def _parse_environment_variables(config_dict):
    """
    Parse environment variables in config values.
    Converts 'env:VARIABLE_NAME' to actual environment variable values.
    """
    if isinstance(config_dict, dict):
        parsed_config = {}
        for key, value in config_dict.items():
            if isinstance(value, str) and value.startswith("env:"):
                env_var = value.split(":", 1)[1]
                env_value = os.environ.get(env_var)
                if env_value:
                    parsed_config[key] = env_value
                    print(f"Loaded {env_var} from environment for {key}")
                else:
                    print(f"Warning: Environment variable {env_var} not found, keeping original value")
                    parsed_config[key] = value
            elif isinstance(value, dict):
                parsed_config[key] = _parse_environment_variables(value)
            else:
                parsed_config[key] = value
        return parsed_config
    return config_dict

def get_memory_client(client_type: str = "vector"):
    global _memory_client

    # 如果已经初始化过，直接返回
    if _memory_client is not None:
        return _memory_client

    try:
        if client_type == "graph":
            config = get_default_graph_config()
            config = _parse_environment_variables(config)
            _memory_client = MemoryGraph.from_config(config)
        elif client_type == "vector":
            config = get_default_vector_config()
            config = _parse_environment_variables(config)
            _memory_client = MemoryVector.from_config(config)
        else:
            raise ValueError("Invalid client type. Use 'graph' or 'vector'.")

        return _memory_client

    except Exception as e:
        print(f"Error initializing memory client: {e}")
        import traceback
        traceback.print_exc()
        _memory_client = None
        return None
