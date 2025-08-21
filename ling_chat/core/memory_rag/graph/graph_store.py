from pydantic import ValidationError
from typing import Any, Dict, Optional
from langchain_community.graphs import Neo4jGraph
import logging

from ling_chat.core.memory_rag.config.base import MemoryConfig
from ling_chat.core.memory_rag.factory import EmbedderFactory, LlmFactory, GraphStoreFactory

logger = logging.getLogger(__name__)

def get_default_graph_config():
    """Get default memory client configuration with sensible defaults."""
    return {
        "graph_store": {
            "provider": "neo4j",
            "config": {
                "url": "env:NEO4J_URL",
                "username": "env:NEO4J_USERNAME",
                "password": "env:NEO4J_PASSWORD",
                "database": "env:NEO4J_DATABASE",
            }
        },
        "llm": {
            "provider": "deepseek",
            "config": {
                "model": "deepseek-chat",
                "temperature": 0.1,
                "max_tokens": 2000,
                "api_key": "env:CHAT_API_KEY"
            }
        },
        "embedder": {
            "provider": "qwen",
            "config": {
                "model": "text-embedding-v4",
                "api_key": "env:DASHSCOPE_API_KEY"
            }
        },
        "version": "v0.3.0"
    }

class MemoryGraph:
    def __init__(self, config: MemoryConfig = MemoryConfig()):
        self.config = config

        self.embedding_model = EmbedderFactory.create(
            self.config.embedder.provider,
            self.config.embedder.config,
            self.config.vector_store.config
        )
        self.graph_store = GraphStoreFactory(self.config)
        self.llm = LlmFactory.create(self.config.llm.provider, self.config.llm.config)
        self.db_name = self.config.graph_store.config.database
        self.api_version = self.config.version
        

    @classmethod
    def from_config(cls, config_dict: Dict[str, Any]):
        try:
            config = cls._process_config(config_dict)
            config = MemoryConfig(**config)
        except ValidationError as e:
            logger.error(f"Configuration validation error: {e}")
            raise
        return cls(config)
    
    @staticmethod
    def _process_config(config_dict: Dict[str, Any]) -> Dict[str, Any]:
        if "graph_store" in config_dict:
            if "vector_store" not in config_dict and "embedder" in config_dict:
                config_dict["vector_store"] = {}
                config_dict["vector_store"]["config"] = {}
        try:
            return config_dict
        except ValidationError as e:
            logger.error(f"Configuration validation error: {e}")
            raise
        
    def add(self, text):
        # Add an item to the graph memory
        print("adding memory")
        return {"status": "success", "message": "Text added to graph memory", "text_length": len(text)}
    
    def _add_to_graph(self, messages, filters):
        added_entities = []
        if self.enable_graph:
            if filters.get("user_id") is None:
                filters["user_id"] = "user"

            data = "\n".join([msg["content"] for msg in messages if "content" in msg and msg["role"] != "system"])
            added_entities = self.graph.add(data, filters)

        return added_entities
    
    def get(self, memory_id):
        pass
    
    def get_all(self):
        pass
    
    def _get_all_from_graph_store(self, filters, limit):
        # Retrieve all items from the vector store with optional filters and limit
        pass
    
    def search(self, query: str):
        pass
    
    def _search_graph_store(self, query, filters, limit, threshold: Optional[float] = None):
        pass
    
    def update(self, memory_id, data):
        pass

    def delete(self, item_id):
        # Delete an item from the graph store
        pass
    
    def delete_all(self, user_id: Optional[str] = None, agent_id: Optional[str] = None, run_id: Optional[str] = None):
        pass
    
    def reset(self):
        pass

    def _create_memory():
        pass

    def _update_memory():
        pass

    def _delete_memory():
        pass
    

