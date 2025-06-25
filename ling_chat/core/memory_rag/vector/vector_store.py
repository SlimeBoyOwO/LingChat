from pydantic import ValidationError
from typing import Any, Dict, Optional
import logging

from core.memory_rag.config.base import MemoryConfig
from core.memory_rag.factory import EmbedderFactory, LlmFactory, VectorStoreFactory

logger = logging.getLogger(__name__)

def get_default_vector_config():
  return {
        "vector_store": {
            "provider": "chroma",
            "config": {
                "collection_name": "openmemory",
                "path": "data/chroma_db_store",
            }
        },
        "llm": {
            "provider": "deepseek",
            "config": {
                "model": "deepseek-chat",
                "temperature": 0.1,
                "max_tokens": 2000,
                # "top_p": 0.9,
                # "top_k": 50,
                "api_key": "env:CHAT_API_KEY"
            }
        },
        "embedder": {
            "provider": "sentence_transformers",
            "config": {
                "model": "all-MiniLM-L6-v2"
                # "api_key": "env:CHAT_API_KEY"
            }
        },
        "version": "v0.3.0"
  }

class MemoryVector:
    def __init__(self, config: MemoryConfig = MemoryConfig()):
        self.config = config

        self.embedding_model = EmbedderFactory.create(
            self.config.embedder.provider,
            self.config.embedder.config,
            self.config.vector_store.config
        )
        self.vector_store = VectorStoreFactory.create(
            self.config.vector_store.provider,
            self.config.vector_store.config
        )
        self.llm = LlmFactory.create(self.config.llm.provider, self.config.llm.config)
        # self.db = SQLiteManager(self.config.history_db_path) # TODO: implement SQLiteManager
        self.collection_name = self.config.vector_store.config.collection_name
        self.api_version = self.config.version

        # TODO：the migration config and initialize function of the vector store

        
    @classmethod
    def from_config(cls, config_dict: Dict[str, Any]):
        try:
            config = cls._process_config(config_dict)
            config = MemoryConfig(**config_dict)
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
                config_dict["vector_store"]["config"]["embedding_model_dims"] = config_dict["embedder"]["config"][
                    "embedding_dims"
                ]
        try:
            return config_dict
        except ValidationError as e:
            logger.error(f"Configuration validation error: {e}")
            raise
        
    def add(self, item):
        # Add an item to the vector store
        print("adding memory")
        return {"status": "success", "message": "Text added to vector memory", "text_length": len(item)}
    
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