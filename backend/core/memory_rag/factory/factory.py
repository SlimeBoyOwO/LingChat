import importlib
import torch
import sys
from typing import Optional

from core.memory_rag.config.base import BaseLlmConfig

def load_class(class_type):
    module_path, class_name = class_type.rsplit(".", 1)
    module = importlib.import_module(module_path)
    return getattr(module, class_name)

class LlmFactory:
    provider_to_class = {
        "deepseek": "core.memory_rag.llms.deepseek.DeepSeekLLM",
        # "ollama": "mem0.llms.ollama.OllamaLLM",
        # "openai": "mem0.llms.openai.OpenAILLM",
        # "groq": "mem0.llms.groq.GroqLLM",
        # "together": "mem0.llms.together.TogetherLLM",
        # "aws_bedrock": "mem0.llms.aws_bedrock.AWSBedrockLLM",
        # "litellm": "mem0.llms.litellm.LiteLLM",
        # "azure_openai": "mem0.llms.azure_openai.AzureOpenAILLM",
        # "openai_structured": "mem0.llms.openai_structured.OpenAIStructuredLLM",
        # "anthropic": "mem0.llms.anthropic.AnthropicLLM",
        # "azure_openai_structured": "mem0.llms.azure_openai_structured.AzureOpenAIStructuredLLM",
        # "gemini": "mem0.llms.gemini.GeminiLLM",
        # "xai": "mem0.llms.xai.XAILLM",
        # "sarvam": "mem0.llms.sarvam.SarvamLLM",
        # "lmstudio": "mem0.llms.lmstudio.LMStudioLLM",
        # "langchain": "mem0.llms.langchain.LangchainLLM",
    }

    @classmethod
    def create(cls, provider_name, config):
        class_type = cls.provider_to_class.get(provider_name)
        if class_type:
            llm_instance = load_class(class_type)
            base_config = BaseLlmConfig(**config)
            return llm_instance(base_config)
        else:
            raise ValueError(f"Unsupported Llm provider: {provider_name}")

class EmbedderFactory:
    provider_to_class = {
        # "openai": "mem0.embeddings.openai.OpenAIEmbedding",
        # "ollama": "mem0.embeddings.ollama.OllamaEmbedding",
        # "huggingface": "mem0.embeddings.huggingface.HuggingFaceEmbedding",
        # "azure_openai": "mem0.embeddings.azure_openai.AzureOpenAIEmbedding",
        # "gemini": "mem0.embeddings.gemini.GoogleGenAIEmbedding",
        # "vertexai": "mem0.embeddings.vertexai.VertexAIEmbedding",
        # "together": "mem0.embeddings.together.TogetherEmbedding",
        # "lmstudio": "mem0.embeddings.lmstudio.LMStudioEmbedding",
        # "langchain": "mem0.embeddings.langchain.LangchainEmbedding",
        # "aws_bedrock": "mem0.embeddings.aws_bedrock.AWSBedrockEmbedding",
    }
    
    @classmethod
    def create(cls, provider_name, config, vector_config: Optional[dict]):
        if provider_name == "sentence_transformers":
            try:
                from sentence_transformers import SentenceTransformer
            except ImportError:
                _sentence_transformer_imported_ok = False
                sys.stderr.write(
                    f"错误: 'sentence-transformers' 模块未找到，但 RAG 功能已启用。\n"
                    f"请安装: pip install sentence-transformers\n")
                sys.stderr.flush()

                class SentenceTransformer:
                    def __init__(self, *args, **kwargs): pass

                    def encode(self, *args, **kwargs): raise NotImplementedError("SentenceTransformer is not available.")
            
            # 处理字典格式的配置
            if isinstance(config, dict):
                model_name = config.get("model", "all-MiniLM-L6-v2")
            else:
                model_name = getattr(config, "model", "all-MiniLM-L6-v2")
            
            device = 'cuda' if torch.cuda.is_available() else 'cpu'
            embedding_model = SentenceTransformer(model_name, device=device)
            return embedding_model
        else:
            raise ValueError(f"Unsupported Embedder provider: {provider_name}, we only support 'sentence_transformers' for now.")
        
class VectorStoreFactory:
    provider_to_class = {
        "chroma": "core.memory_rag.vector.chroma.ChromaDB",
        # "qdrant": "mem0.vector_stores.qdrant.Qdrant",
        # "pgvector": "mem0.vector_stores.pgvector.PGVector",
        # "milvus": "mem0.vector_stores.milvus.MilvusDB",
        # "upstash_vector": "mem0.vector_stores.upstash_vector.UpstashVector",
        # "azure_ai_search": "mem0.vector_stores.azure_ai_search.AzureAISearch",
        # "pinecone": "mem0.vector_stores.pinecone.PineconeDB",
        # "mongodb": "mem0.vector_stores.mongodb.MongoDB",
        # "redis": "mem0.vector_stores.redis.RedisDB",
        # "elasticsearch": "mem0.vector_stores.elasticsearch.ElasticsearchDB",
        # "vertex_ai_vector_search": "mem0.vector_stores.vertex_ai_vector_search.GoogleMatchingEngine",
        # "opensearch": "mem0.vector_stores.opensearch.OpenSearchDB",
        # "supabase": "mem0.vector_stores.supabase.Supabase",
        # "weaviate": "mem0.vector_stores.weaviate.Weaviate",
        # "faiss": "mem0.vector_stores.faiss.FAISS",
        # "langchain": "mem0.vector_stores.langchain.Langchain",
    }
    @classmethod
    def create(cls, provider_name, config):
        class_type = cls.provider_to_class.get(provider_name)
        if class_type:
            if not isinstance(config, dict):
                # 检查是否有model_dump方法（Pydantic模型）
                if hasattr(config, 'model_dump'):
                    config = config.model_dump()
                else:
                    # 对于非Pydantic模型（如ChromaDB），直接返回实例
                    return config
            vector_store_instance = load_class(class_type)
            return vector_store_instance(**config)
        else:
            raise ValueError(f"Unsupported VectorStore provider: {provider_name}")

    @classmethod
    def reset(cls, instance):
        instance.reset()
        return instance