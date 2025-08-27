# neochat/llm/manager.py
from neochat.platform.configuration import config
from neochat.platform.logging import log_info, log_error
from neochat.llm.clients.base import BaseLLMClient
from neochat.llm.clients.openai_compatible import OpenAICompatibleClient

# 客户端类型与实现类的映射
CLIENT_MAPPING = {
    "openai_compatible": OpenAICompatibleClient,
    # 未来可以添加更多非 OpenAI 兼容的客户端类型
    # "gemini": GeminiClient,
}

class LLMManager:
    """
    管理和提供 LLM 客户端实例。
    """
    def __init__(self):
        self._client: BaseLLMClient = None
        self._initialize_client()

    def _initialize_client(self):
        """根据配置初始化当前的 LLM 客户端。"""
        provider_name = config.llm.active_provider
        provider_config = getattr(config.llm.providers, provider_name, None)

        if not provider_config:
            log_error(f"配置错误: 在 config.yaml 中找不到名为 '{provider_name}' 的 LLM provider。")
            raise ValueError(f"Invalid LLM provider: {provider_name}")

        client_type = getattr(provider_config, 'type', None)
        ClientClass = CLIENT_MAPPING.get(client_type)

        if not ClientClass:
            log_error(f"配置错误: 不支持的客户端类型 '{client_type}'。")
            raise ValueError(f"Unsupported client type: {client_type}")

        # 将配置对象转换为字典传递给客户端
        config_dict = {k: v for k, v in vars(provider_config).items() if not k.startswith('_')}
        
        self._client = ClientClass(config_dict)
        log_info(f"LLM 管理器已初始化，当前服务商: '{provider_name}' ({client_type})")

    @property
    def client(self) -> BaseLLMClient:
        """获取当前激活的 LLM 客户端实例。"""
        if not self._client:
            raise RuntimeError("LLM 客户端未初始化。")
        return self._client

# 创建一个全局单例
llm_manager = LLMManager()