from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.llm_providers.gemini import GeminiProvider
from ling_chat.core.llm_providers.kimi_code import KimiCodeProvider
from ling_chat.core.llm_providers.lmstudio import LMStudioProvider
from ling_chat.core.llm_providers.ollama import OllamaProvider
from ling_chat.core.llm_providers.qwen_translate import QwenTranslateProvider
from ling_chat.core.llm_providers.web_llm import WebLLMProvider
from ling_chat.core.logger import logger


class LLMProviderFactory:
    @staticmethod
    def create_provider(
        provider_type: str, model_type: str = "", api_key: str = "", base_url: str = "", **kwargs
    ) -> BaseLLMProvider:
        """
        创建指定类型的大模型提供者

        :param provider_type: 提供者类型 (webllm, ollama, lmstudio, gemini, qwen, kimi-code)
        :param model_type: 模型名称
        :param api_key: API 密钥
        :param base_url: API 访问地址
        :return: 大模型提供者实例

        网络代理由全局 ``[network]`` 配置统一管理，无需在此传入。
        """
        # 兼容旧调用：忽略遗留的 proxy 参数
        kwargs.pop("proxy", None)

        provider_type = provider_type.lower()

        try:
            if provider_type == "webllm":
                logger.info("创建通用联网大模型服务提供商")
                return WebLLMProvider(model_type, api_key, base_url)
            elif provider_type == "ollama":
                logger.info("创建OLLAMA服务提供商")
                return OllamaProvider(model_type, api_key, base_url)
            elif provider_type == "lmstudio":
                logger.info("创建LM STUDIO服务提供商")
                return LMStudioProvider(model_type, api_key, base_url)
            elif provider_type == "gemini":
                logger.info("创建Gemini服务提供商")
                return GeminiProvider(model_type, api_key, base_url)
            elif provider_type == "kimi-code":
                logger.info("创建 Kimi Code 服务提供商")
                return KimiCodeProvider(model_type, api_key, base_url)
            elif provider_type == "qwen-translate":
                logger.info("创建Qwen翻译服务提供商")
                return QwenTranslateProvider()
            else:
                raise ValueError(f"暂未支持的提供商: {provider_type}")
        except Exception as e:
            logger.error(f"创建服务提供商 {provider_type} 失败: {str(e)}")
            raise
