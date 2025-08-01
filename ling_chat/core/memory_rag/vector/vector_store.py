import json
from pydantic import ValidationError
from typing import Any, Dict, Optional
import logging

from core.memory_rag.utils import *
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
            "provider": "qwen",
            "config": {
                "model": "text-embedding-v4",
                "api_key": "env:DASHSCOPE_API_KEY"
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
        
    def add(
        self, 
        messages, 
        user_id: Optional[str] = None, 
        character_id: Optional[str] = None, 
        metadata: Optional[Dict[str, Any]] = None,
        infer: bool = False):
        # build filters


        # check messages type
        if isinstance(messages, str):
            messages = [{"role": "user", "content": messages}]

        elif isinstance(messages, dict):
            messages = [messages]

        elif not isinstance(messages, list):
            raise ValueError("messages must be str, dict, or list[dict]")
        
        # add memory data
        vector_store_result = future1.result()

        logger.info("adding vector memory")
        return {"results": vector_store_result}
    
    def _add_to_vector_store(self, messages, metadata, filters, infer):
        if not infer:
            pass

        parsed_messages = parse_messages(messages)
        system_prompt, user_prompt = get_fact_retrieval_messages(parsed_messages)

        response = self.llm.generate_response(
            messages=[
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt},
            ],
            response_format={"type": "json_object"},
        )

        try:
            response = remove_code_blocks(response)
            new_retrieved_facts = json.loads(response)["facts"]
        except Exception as e:
            logging.error(f"Error in new_retrieved_facts: {e}")
            new_retrieved_facts = []

        if not new_retrieved_facts:
            logger.debug("No new facts retrieved from input. Skipping memory update LLM call.")

        
    
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

    def _create_memory():
        pass

    def _update_memory():
        pass

    def _delete_memory():
        pass