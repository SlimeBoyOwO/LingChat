"""网络代理配置工具

为各 LLM Provider 提供统一的代理配置读取与 httpx 客户端构造能力。

读取优先级：Provider 专属 *_PROXY_URL > GLOBAL_PROXY_URL > None。
留空则视为不开启代理，行为与未配置代理时完全一致。
"""

import os
from typing import Optional

import httpx


def get_proxy_url(provider_key: str) -> Optional[str]:
    """获取指定 Provider 的代理 URL，支持 fallback 到全局代理。

    优先级：Provider 专属（如 LLM_PROXY_URL） > GLOBAL_PROXY_URL > None
    """
    specific = os.environ.get(provider_key, "").strip()
    if specific:
        return specific
    global_proxy = os.environ.get("GLOBAL_PROXY_URL", "").strip()
    return global_proxy if global_proxy else None


def create_proxied_httpx_client(
    proxy_url: Optional[str], timeout: Optional[httpx.Timeout] = None
) -> Optional[httpx.Client]:
    """创建带代理的 httpx.Client。

    proxy_url 为空则返回 None（让上层 SDK 使用其默认 client，避免破坏既有行为）。
    """
    if not proxy_url:
        return None
    kwargs = {"proxy": proxy_url, "trust_env": False}
    if timeout is not None:
        kwargs["timeout"] = timeout
    return httpx.Client(**kwargs)


def create_proxied_async_httpx_client(
    proxy_url: Optional[str], timeout: Optional[httpx.Timeout] = None
) -> Optional[httpx.AsyncClient]:
    """创建带代理的 httpx.AsyncClient。

    proxy_url 为空则返回 None。
    """
    if not proxy_url:
        return None
    kwargs = {"proxy": proxy_url, "trust_env": False}
    if timeout is not None:
        kwargs["timeout"] = timeout
    return httpx.AsyncClient(**kwargs)
