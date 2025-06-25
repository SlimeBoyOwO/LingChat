import os
from typing import Any, Dict, Optional

from pydantic import BaseModel, Field

from .llms import BaseLlmConfig

from .embedder_config import EmbedderConfig
from .vector_config import VectorStoreConfig
from .graph_config import LlmConfig, GraphStoreConfig

class MemoryConfig(BaseModel):
    vector_store: VectorStoreConfig = Field(
        description="Configuration for the vector store",
        default_factory=VectorStoreConfig,
    )
    llm: LlmConfig = Field(
        description="Configuration for the language model",
        default_factory=LlmConfig,
    )
    embedder: EmbedderConfig = Field(
        description="Configuration for the embedding model",
        default_factory=EmbedderConfig,
    )
    graph_store: GraphStoreConfig = Field(
        description="Configuration for the graph",
        default_factory=GraphStoreConfig,
    )
    version: str = Field(
        description="The version of the API",
        default="v1.1",
    )
    # custom_fact_extraction_prompt: Optional[str] = Field(
    #     description="Custom prompt for the fact extraction",
    #     default=None,
    # )
    # custom_update_memory_prompt: Optional[str] = Field(
    #     description="Custom prompt for the update memory",
    #     default=None,
    # )