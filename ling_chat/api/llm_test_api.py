"""LLM 配置测试 API — 发送测试消息验证配置是否可用"""

from typing import Any, Dict, Optional

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, Field

from ling_chat.core.llm_providers.provider_factory import LLMProviderFactory
from ling_chat.core.logger import logger

router = APIRouter(prefix="/api/v1/llm-config", tags=["LLM Config"])


class TestProviderRequest(BaseModel):
    provider: str = Field(..., description="提供商类型 (webllm/gemini/ollama/lmstudio)")
    model: str = Field(default="", description="模型名称")
    api_key: str = Field(default="", description="API 密钥")
    base_url: str = Field(default="", description="API 地址")
    temperature: Optional[float] = Field(default=None, description="Temperature 参数")
    top_p: Optional[float] = Field(default=None, description="Top P 参数")
    enable_thinking: str = Field(default="none", description="是否启用思考链 (none/true/false)")
    message: str = Field(default="只需回复两个字：你好", description="测试消息")

    model_config = {"extra": "ignore"}


@router.post("/test")
def test_llm_provider(request: TestProviderRequest) -> Dict[str, Any]:
    """测试 LLM 提供商连接"""
    try:
        logger.info(f"测试 LLM 提供商: {request.provider}/{request.model}")

        provider = LLMProviderFactory.create_provider(
            provider_type=request.provider,
            model_type=request.model,
            api_key=request.api_key,
            base_url=request.base_url,
        )

        messages = [
            {"role": "user", "content": request.message},
        ]

        response = provider.generate_response(messages)

        return {
            "status": "success",
            "response": response,
        }

    except Exception as e:
        logger.error(f"测试 LLM 提供商失败: {e}")
        return {
            "status": "error",
            "response": str(e),
        }
