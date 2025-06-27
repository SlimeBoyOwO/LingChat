import unittest
import sys
from dotenv import load_dotenv
import os

# 添加项目根目录到 Python 路径
project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, project_root)

from core.memory_rag.factory import EmbedderFactory
from core.memory_rag.vector.vector_store import get_default_vector_config
from core.memory_client import _parse_environment_variables

def main():
    # Get the default vector configuration
    config = get_default_vector_config()
    
    # Parse environment variables in the configuration
    config = _parse_environment_variables(config)
    
    print("Default Vector Configuration:")
    print(config)

    # Test EmbedderFactory with the default configuration
    embedder = EmbedderFactory.create(
        config['embedder']['provider'],
        config['embedder']['config'],
        config['vector_store']['config']
    )
    
    print("Embedder created successfully:", embedder)

    # run a embed function
    text = "测试文本"
    embedding = embedder.embed(text)
    print("Embedding for text:", embedding)

if __name__ == "__main__":
  load_dotenv()
  main()