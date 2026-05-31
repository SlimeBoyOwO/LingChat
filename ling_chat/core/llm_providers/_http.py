"""HTTP 客户端工厂 — 统一处理全局代理配置

设计要点：
- 全局代理配置位于 [network] 段，所有 provider 共用
- 留空走系统代理（httpx 默认 trust_env=True）
- 配了代理则强制走该代理（trust_env=False）
- 当 base_url 指向本地地址时强制不走代理（避免 Ollama/LMStudio 等被中断）

⚠️ 循环依赖约束
- ``build_httpx_client`` 内部延迟导入 ``llm_config`` 以避免循环依赖
- 所有模块级 ``from ._http import build_httpx_client`` 的模块
  必须在 ``llm_config`` 初始化之后才被实际加载（Python 只在首次调用函数时
  才会真正执行 import）——当前所有 provider 通过 ``LLMProviderFactory
  .create_provider()`` 懒加载，故安全。
- 如果未来有人在模块顶层（如 ``__init__.py``）直接 import provider 且该
  模块早于 ``llm_config`` 被加载，则会导致循环导入，请用函数内延迟导入代替。
"""

from typing import Any, Optional, Union
from urllib.parse import urlparse

import httpx

_LOCAL_HOSTS = {"localhost", "127.0.0.1", "::1", "0.0.0.0"}


def _is_local_url(url: Optional[str]) -> bool:
    """判断 URL 是否指向本机地址

    覆盖：
    - 精确匹配：localhost、127.0.0.1、::1、0.0.0.0
    - 后缀匹配：*.localhost（如 mymodel.localhost）
    - 大小写不敏感
    """
    if not url:
        return False
    try:
        host = urlparse(url).hostname or ""
    except Exception:
        return False
    host = host.lower()
    if host in _LOCAL_HOSTS:
        return True
    # 匹配 *.localhost 变体（如 llama.localhost）
    if host.endswith(".localhost"):
        return True
    return False


def build_httpx_client(
    *,
    async_client: bool = False,
    timeout: Any = None,
    base_url: Optional[str] = None,
    **kwargs: Any,
) -> Union[httpx.Client, httpx.AsyncClient]:
    """构造 httpx 客户端，统一处理全局代理。

    行为：
    - 配置了全局代理且 base_url 非本地 → ``proxy=值, trust_env=False``
    - 留空 → 使用 httpx 默认（``trust_env=True``，识别系统 ``HTTP_PROXY/HTTPS_PROXY``）
    - base_url 指向本地（localhost / 127.0.0.1 / ::1 / 0.0.0.0）→ 不走代理

    Args:
        async_client: 是否构造 ``httpx.AsyncClient``，默认 ``httpx.Client``
        timeout: httpx 超时配置
        base_url: 既用于判断是否本地连接，也会作为 httpx 客户端的 ``base_url``
            参数透传（仅在请求使用相对路径时生效，对绝对 URL 请求无影响）。
            传 ``None`` 时不设置。
        **kwargs: 透传给 httpx.Client/AsyncClient 的其他参数

    Returns:
        ``httpx.Client`` 或 ``httpx.AsyncClient`` 实例
    """
    # 延迟导入避免与 llm_config 形成循环依赖
    from ling_chat.configs.llm_config import llm_config

    proxy = (llm_config.get_network_config().get("proxy") or "").strip()

    cls = httpx.AsyncClient if async_client else httpx.Client
    init_kwargs: dict = dict(kwargs)
    if timeout is not None:
        init_kwargs["timeout"] = timeout
    if base_url:
        init_kwargs.setdefault("base_url", base_url)

    if proxy and not _is_local_url(base_url):
        init_kwargs["proxy"] = proxy
        init_kwargs["trust_env"] = False

    return cls(**init_kwargs)
