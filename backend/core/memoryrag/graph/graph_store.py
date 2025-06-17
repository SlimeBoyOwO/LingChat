import os
from typing import Any, Dict, Optional
from langchain_community.graphs import Neo4jGraph

from memoryrag.config.base import MemoryConfig

def get_default_graph_config():
    NEO4J_URL = os.getenv("NEO4J_URL", "bolt://localhost:7687")
    NEO4J_USERNAME = os.getenv("NEO4J_USERNAME", "neo4j")
    NEO4J_PASSWORD = os.getenv("NEO4J_PASSWORD", "neo4j")
    """Get default memory client configuration with sensible defaults."""
    return {
        "graph_store": {
            "provider": "neo4j",
            "config": {
                "url": f"{NEO4J_URL}",
                "username": f"{NEO4J_USERNAME}",
                "password": f"{NEO4J_PASSWORD}" # TODO: Use env variable
            }
        },
        # "llm": {
        #     "provider": "openai",
        #     "config": {
        #         "model": "gpt-4o-mini",
        #         "temperature": 0.1,
        #         "max_tokens": 2000,
        #         "api_key": "env:OPENAI_API_KEY" # TODO: Use env variable
        #     }
        # },
        # "embedder": {
        #     "provider": "openai",
        #     "config": {
        #         "model": "text-embedding-3-small",
        #         "api_key": "env:OPENAI_API_KEY" # TODO: Use env variable
        #     }
        # },
        "version": "v0.1.0"
    }

class MemoryGraph:
    def __init__(self, config: MemoryConfig = MemoryConfig()):
        self.config = config
        

    @classmethod
    def from_config(cls, config_dict: Dict[str, Any]):
        pass
    
    @staticmethod
    def _process_config(config_dict: Dict[str, Any]) -> Dict[str, Any]:
        pass
        
    def add(self, item):
        # Add an item to the vector store
        pass
    
    def _add_to_vector_store(self, messages, metadata, filters, infer):
        pass
    
    def _add_to_graph(self, messages, filters):
        pass
    
    def get(self, memory_id):
        pass
    
    def get_all(self):
        pass
    
    def _get_all_from_vector_store(self, filters, limit):
        # Retrieve all items from the vector store with optional filters and limit
        pass
    
    def search(self, query: str):
        pass
    
    def _search_vector_store(self, query, filters, limit, threshold: Optional[float] = None):
        pass
    
    def update(self, memory_id, data):
        pass

    def delete(self, item_id):
        # Delete an item from the vector store
        pass
    
    def delete_all(self, user_id: Optional[str] = None, agent_id: Optional[str] = None, run_id: Optional[str] = None):
        pass
    
    def reset(self):
        pass
    

