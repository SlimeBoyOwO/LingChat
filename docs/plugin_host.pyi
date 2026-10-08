"""LingChat 插件宿主模块 `plugin_host` 的类型存根。

本文件只给编辑器 / 类型检查器看：运行时不加载它，宿主会把真正的 `plugin_host`
原生模块注入解释器，插件里 `from plugin_host import ...` 照常可用。

用法：把本文件拷进插件目录（与 .py 脚本同级），或把它所在目录加入 IDE 的
extraPaths（Pylance）/ stubPath，即可获得补全与类型检查。

所有函数都不抛异常，统一返回 dict：成功含 `ok: True`，失败含
`ok: False` 与 `error: str`（字段释义见 plugin-dev-guide.md）。
"""

from typing import Any, Dict, List, Optional

def http_get(
    url: str,
    *,
    query: Optional[Dict[str, Any]] = ...,
    headers: Optional[Dict[str, str]] = ...,
    timeout_ms: int = ...,
) -> Dict[str, Any]:
    """HTTP GET。返回 {status, ok, body}；失败 {ok: False, error}。"""

def http_post(
    url: str,
    *,
    headers: Optional[Dict[str, str]] = ...,
    body: Any = ...,
    timeout_ms: int = ...,
) -> Dict[str, Any]:
    """HTTP POST（body 自动 JSON 序列化）。返回同 http_get。"""

def read_data_file(path: str) -> Dict[str, Any]:
    """读 data/ 下白名单内的文件。成功 {ok, size, base64}，失败 {ok: False, error}。"""

def send_user_message(text: str) -> Dict[str, Any]:
    """把外部消息当成玩家发言送进对话（后台生成回复）。需 manifest 声明
    send_user_message = true。成功 {ok}，失败 {ok: False, error}。"""

def switch_character(role_id: int) -> Dict[str, Any]:
    """完整切换当前角色（会清空对话历史）。成功 {ok, role_id, name}。"""

def read_context(
    *,
    start: Optional[int] = ...,
    end: Optional[int] = ...,
) -> Dict[str, Any]:
    """读台词历史第 start~end 条，缺省整段。返回 {ok, total, lines}。"""

def edit_context(
    replacement: List[Dict[str, Any]],
    *,
    start: Optional[int] = ...,
    end: Optional[int] = ...,
) -> Dict[str, Any]:
    """用 replacement 替换第 start~end 条台词。返回 {ok, removed, total}。"""

def compress_context() -> Dict[str, Any]:
    """触发并等待一次永久记忆压缩。返回 {ok, triggered}。"""

def ws_open(conn_id: str, *, url: Optional[str] = ...) -> Dict[str, Any]:
    """启用一条已声明的 WS 连接（可选 url 覆盖声明值）。返回 {ok}。"""

def ws_close(conn_id: str) -> Dict[str, Any]:
    """关闭一条 WS 连接。返回 {ok}。"""

def ws_send(conn_id: str, data: Any) -> Dict[str, Any]:
    """向 WS 连接发一帧：str → 文本，bytes → 二进制，其余 → JSON 文本。返回 {ok}。"""

def ws_status(*, conn_id: Optional[str] = ...) -> Dict[str, Any]:
    """查询 WS 连接状态。返回 {ok, connections} 或单条 {ok, id, mode, state}。"""
