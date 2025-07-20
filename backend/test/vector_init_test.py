import sys
from dotenv import load_dotenv
import os

# 添加项目根目录到 Python 路径
project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, project_root)

from core.memory_client import _parse_environment_variables
from core.memory_rag.vector import MemoryVector, get_default_vector_config

def test_memory_vector_initialization():
    # Get the default vector configuration
    config = get_default_vector_config()
    
    # Parse environment variables in the configuration
    parsed_config = _parse_environment_variables(config)
    
    # Initialize MemoryVector with the parsed configuration
    memory_vector = MemoryVector.from_config(parsed_config)
    
    # Check if the initialization was successful
    assert memory_vector is not None, "MemoryVector initialization failed"
    
    # Check if the embedding model is set correctly
    assert memory_vector.embedding_model is not None, "Embedding model is not initialized"
    
    # Check if the vector store is set correctly
    assert memory_vector.vector_store is not None, "Vector store is not initialized"
    
    # Check if the LLM is set correctly
    assert memory_vector.llm is not None, "LLM is not initialized"
    
    print("MemoryVector initialization test passed.")

if __name__ == "__main__":
    load_dotenv()
    test_memory_vector_initialization()
    print("All tests passed.")