import logging
from typing import Any, ClassVar, Dict, List, Optional

import chromadb
from chromadb.config import Settings

from pydantic import BaseModel, Field, model_validator

logger = logging.getLogger(__name__)

class ChromaDbConfig(BaseModel):
    try:
        from chromadb.api.client import Client
    except ImportError:
        raise ImportError("The 'chromadb' library is required. Please install it using 'pip install chromadb'.")
    Client: ClassVar[type] = Client

    collection_name: str = Field("mem0", description="Default name for the collection")
    client: Optional[Client] = Field(None, description="Existing ChromaDB client instance")
    path: Optional[str] = Field(None, description="Path to the database directory")
    host: Optional[str] = Field(None, description="Database connection remote host")
    port: Optional[int] = Field(None, description="Database connection remote port")

    @model_validator(mode="before")
    def check_host_port_or_path(cls, values):
        host, port, path = values.get("host"), values.get("port"), values.get("path")
        if not path and not (host and port):
            raise ValueError("Either 'host' and 'port' or 'path' must be provided.")
        return values

    @model_validator(mode="before")
    @classmethod
    def validate_extra_fields(cls, values: Dict[str, Any]) -> Dict[str, Any]:
        allowed_fields = set(cls.model_fields.keys())
        input_fields = set(values.keys())
        extra_fields = input_fields - allowed_fields
        if extra_fields:
            raise ValueError(
                f"Extra fields not allowed: {', '.join(extra_fields)}. Please input only the following fields: {', '.join(allowed_fields)}"
            )
        return values

    model_config = {
        "arbitrary_types_allowed": True,
    }

class OutputData(BaseModel):
    id: Optional[str]  # memory id
    score: Optional[float]  # distance
    payload: Optional[Dict]  # metadata

class ChromaDB():
    def __init__(
        self,
        collection_name: str,
        client: Optional[chromadb.Client] = None,
        host: Optional[str] = None,
        port: Optional[int] = None,
        path: Optional[str] = None,
    ):
        """
        Initialize the Chromadb vector store.

        Args:
            collection_name (str): Name of the collection.
            client (chromadb.Client, optional): Existing chromadb client instance. Defaults to None.
            host (str, optional): Host address for chromadb server. Defaults to None.
            port (int, optional): Port for chromadb server. Defaults to None.
            path (str, optional): Path for local chromadb database. Defaults to None.
        """
        if client:
            self.client = client
        else:
            self.settings = Settings(anonymized_telemetry=False)

            if host and port:
                # 如果指定了 host 和 port，使用 HttpClient
                self.client = chromadb.HttpClient(host=host, port=port)
            else:
                # 使用 PersistentClient 替代 Client + Settings
                if path is None:
                    path = "db"
                self.client = chromadb.PersistentClient(path=path)

        self.collection_name = collection_name
        self.collection = self.create_col(collection_name)

    def _parse_output(self, data: Dict) -> List[OutputData]:
        """
        Parse the output data.

        Args:
            data (Dict): Output data.

        Returns:
            List[OutputData]: Parsed output data.
        """
        keys = ["ids", "distances", "metadatas"]
        values = []

        for key in keys:
            value = data.get(key, [])
            if isinstance(value, list) and value and isinstance(value[0], list):
                value = value[0]
            values.append(value)

        ids, distances, metadatas = values
        max_length = max(len(v) for v in values if isinstance(v, list) and v is not None)

        result = []
        for i in range(max_length):
            entry = OutputData(
                id=ids[i] if isinstance(ids, list) and ids and i < len(ids) else None,
                score=(distances[i] if isinstance(distances, list) and distances and i < len(distances) else None),
                payload=(metadatas[i] if isinstance(metadatas, list) and metadatas and i < len(metadatas) else None),
            )
            result.append(entry)

        return result

    def create_col(self, name: str, embedding_fn: Optional[callable] = None):
        """
        Create a new collection.

        Args:
            name (str): Name of the collection.
            embedding_fn (Optional[callable]): Embedding function to use. Defaults to None.

        Returns:
            chromadb.Collection: The created or retrieved collection.
        """
        collection = self.client.get_or_create_collection(
            name=name,
            embedding_function=embedding_fn,
            metadata={"hnsw:space": "cosine"}
        )
        return collection

    def insert(
        self,
        vectors: List[list],
        payloads: Optional[List[Dict]] = None,
        ids: Optional[List[str]] = None,
    ):
        """
        Insert vectors into a collection.

        Args:
            vectors (List[list]): List of vectors to insert.
            payloads (Optional[List[Dict]], optional): List of payloads corresponding to vectors. Defaults to None.
            ids (Optional[List[str]], optional): List of IDs corresponding to vectors. Defaults to None.
        """
        logger.info(f"Inserting {len(vectors)} vectors into collection {self.collection_name}")
        self.collection.add(ids=ids, embeddings=vectors, metadatas=payloads)

    def search(
        self, query: str, vectors: List[list], limit: int = 5, filters: Optional[Dict] = None
    ) -> List[OutputData]:
        """
        Search for similar vectors.

        Args:
            query (str): Query.
            vectors (List[list]): List of vectors to search.
            limit (int, optional): Number of results to return. Defaults to 5.
            filters (Optional[Dict], optional): Filters to apply to the search. Defaults to None.

        Returns:
            List[OutputData]: Search results.
        """
        results = self.collection.query(query_embeddings=vectors, where=filters, n_results=limit)
        final_results = self._parse_output(results)
        return final_results

    def delete(self, vector_id: str):
        """
        Delete a vector by ID.

        Args:
            vector_id (str): ID of the vector to delete.
        """
        self.collection.delete(ids=vector_id)

    def update(
        self,
        vector_id: str,
        vector: Optional[List[float]] = None,
        payload: Optional[Dict] = None,
    ):
        """
        Update a vector and its payload.

        Args:
            vector_id (str): ID of the vector to update.
            vector (Optional[List[float]], optional): Updated vector. Defaults to None.
            payload (Optional[Dict], optional): Updated payload. Defaults to None.
        """
        self.collection.update(ids=vector_id, embeddings=vector, metadatas=payload)

    def get(self, vector_id: str) -> OutputData:
        """
        Retrieve a vector by ID.

        Args:
            vector_id (str): ID of the vector to retrieve.

        Returns:
            OutputData: Retrieved vector.
        """
        result = self.collection.get(ids=[vector_id])
        return self._parse_output(result)[0]

    def list_cols(self) -> List[chromadb.Collection]:
        """
        List all collections.

        Returns:
            List[chromadb.Collection]: List of collections.
        """
        return self.client.list_collections()

    def delete_col(self):
        """
        Delete a collection.
        """
        self.client.delete_collection(name=self.collection_name)

    def col_info(self) -> Dict:
        """
        Get information about a collection.

        Returns:
            Dict: Collection information.
        """
        return self.client.get_collection(name=self.collection_name)

    def list(self, filters: Optional[Dict] = None, limit: int = 100) -> List[OutputData]:
        """
        List all vectors in a collection.

        Args:
            filters (Optional[Dict], optional): Filters to apply to the list. Defaults to None.
            limit (int, optional): Number of vectors to return. Defaults to 100.

        Returns:
            List[OutputData]: List of vectors.
        """
        results = self.collection.get(where=filters, limit=limit)
        return [self._parse_output(results)]

    def reset(self):
        """Reset the index by deleting and recreating it."""
        logger.warning(f"Resetting index {self.collection_name}...")
        self.delete_col()
        self.collection = self.create_col(self.collection_name)


class VectorStoreConfig(BaseModel):
    provider: str = Field(
        description="Provider of the vector store (e.g., 'qdrant', 'chroma', 'upstash_vector')",
        default="chroma",
    )
    config: Optional[Dict] = Field(
        description="Configuration for the specific vector store", 
        default_factory=lambda: {"collection_name": "openmemory"}
    )

    _provider_configs: Dict[str, str] = {
        # "qdrant": "QdrantConfig",
        "chroma": "ChromaDbConfig",
        # "pgvector": "PGVectorConfig",
        # "pinecone": "PineconeConfig",
        # "mongodb": "MongoDBConfig",
        # "milvus": "MilvusDBConfig",
        # "upstash_vector": "UpstashVectorConfig",
        # "azure_ai_search": "AzureAISearchConfig",
        # "redis": "RedisDBConfig",
        # "elasticsearch": "ElasticsearchConfig",
        # "vertex_ai_vector_search": "GoogleMatchingEngineConfig",
        # "opensearch": "OpenSearchConfig",
        # "supabase": "SupabaseConfig",
        # "weaviate": "WeaviateConfig",
        # "faiss": "FAISSConfig",
        # "langchain": "LangchainConfig",
    }

    @model_validator(mode="after")
    def validate_and_create_config(self) -> "VectorStoreConfig":
        provider = self.provider
        config = self.config

        if provider not in self._provider_configs:
            raise ValueError(f"Unsupported vector store provider: {provider}")

        # 直接使用同一文件中的ChromaDbConfig类
        if provider == "chroma":
            config_class = ChromaDbConfig
        else:
            # 对于其他provider，保持原有的导入方式
            module = __import__(
                f"{provider}",
                fromlist=[self._provider_configs[provider]],
            )
            config_class = getattr(module, self._provider_configs[provider])

        if config is None:
            config = {}

        if not isinstance(config, dict):
            if not isinstance(config, config_class):
                raise ValueError(f"Invalid config type for provider {provider}")
            return self
        
        # also check if path in allowed keys for pydantic model, and whether config extra fields are allowed
        if "path" not in config and "path" in config_class.__annotations__:
            config["path"] = "data/chroma_db_store"
        
        self.config = config_class(**config)
        return self