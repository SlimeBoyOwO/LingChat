import sys
from dotenv import load_dotenv
import os

# 添加项目根目录到 Python 路径
project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, project_root)

from ling_chat.core.memory_rag.vector.vector_store import get_default_vector_config
from ling_chat.core.memory_rag.graph.graph_store import get_default_graph_config
from ling_chat.core.memory_client import _parse_environment_variables

def main():
    # Get the default vector configuration
    config = get_default_vector_config()

    # Parse environment variables in the configuration
    config = _parse_environment_variables(config)

    print("Default Vector Configuration:")
    print(config)

    # Get the default graph configuration
    graph_config = get_default_graph_config()
    # Parse environment variables in the graph configuration
    graph_config = _parse_environment_variables(graph_config)
    print("\nDefault Graph Configuration:")
    print(graph_config)


if __name__ == "__main__":
    load_dotenv()
    main()