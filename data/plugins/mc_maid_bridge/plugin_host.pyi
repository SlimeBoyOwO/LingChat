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

def capture_screen(*, window: str = ...) -> Dict[str, Any]:
    """抓一张屏幕画面，返回 JPEG 的 data URL。window 是要找的窗口标题（或进程名）
    关键词，找不到就抓整个桌面；图会缩到长边 1024 再编 JPEG。需 manifest 声明
    capture_screen = true，且当前对话模型开着原生识图。
    成功 {ok, image}，失败 {ok: False, error}。"""

def switch_character(role_id: int) -> Dict[str, Any]:
    """完整切换当前角色（会清空对话历史）。成功 {ok, role_id, name}。"""

def current_names() -> Dict[str, Any]:
    """取当前角色名与玩家名（写旁白时用第三人称称呼）。
    成功 {ok, role_id, ai_name, user_name}；没加载角色时 ai_name 为空串。"""

def set_auto_mode(enabled: bool) -> Dict[str, Any]:
    """切换主界面的「自动模式」（自动逐句推进对话）。不落盘，断开不还原。成功 {ok}。"""

def set_reply_length_mode(mode: str) -> Dict[str, Any]:
    """切换回复台词数量档位：short（1~2 句）/ normal（3~5 句）/ free（不限制）。
    要求已经进入对话。档位没变时不写台词，重复调用安全。
    成功 {ok, mode}；失败 {ok: False, error}。"""

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

def add_prompt_line(
    role: str,
    text: str,
    *,
    ttl_seconds: Optional[float] = ...,
    key: Optional[str] = ...,
    severity: int = ...,
) -> Dict[str, Any]:
    """往台词表末尾追加一条提示台词（旁白/系统/剧情），不触发回复。
    role 取 "narrator" / "system" / "plot"。
    空闲时返回 {ok, index}；生成进行中先排队，返回 {ok, queued: True}，
    下一轮开头再按顺序写进台词表（连着的 narrator 旁白会并成一行）。
    传 ttl_seconds 表示这条只在这么长时间内有效：先留在队列里，等角色真要开口时
    再判一次，过期的直接丢弃、不写进台词表；key + severity 让同类事件（比如连着
    挨打）只保留最严重的一条。带有效期的调用返回 {ok, queued: True, pending: N}，
    pending 是队列里还有效的条目数。"""

def trigger_proactive(text: str, *, urgent: bool = ...) -> Dict[str, Any]:
    """请求宿主主动开一次口（主动对话系统的一次投放）。需 manifest 声明
    trigger_proactive = true。请求只排队：等这一轮台词播完、再过冷却才投放；
    同一时刻最多攒一条，连发会合并。{ok, triggered: True} = 已受理/已合并；
    proactive_disabled / system_busy 见指南。
    text 为空 = 只开口、不往台词表写提示行。
    urgent = 死亡这类必须马上开口的事：顶掉排队中的普通请求，也不吃冷却与静默期。"""

def set_voice_playback_available(available: bool) -> Dict[str, Any]:
    """报告外部程序此刻能不能播这句语音（用于 [voice_playback] 接管期间）。
    女仆倒下、模组找不到播放目标，就传 False，宿主下一句改回本机播放；
    复活或重连时再传 True。返回 {ok}。"""

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

def ws_request(
    conn_id: str, data: Dict[str, Any], *, timeout_ms: Optional[int] = ...
) -> Dict[str, Any]:
    """发一条请求并等它的响应帧（data 必须是对象，宿主补 request_id）。

    会阻塞到响应或超时，timeout_ms 要明显小于工具自身的 timeout_ms。
    返回 {ok, response} 或 {ok: False, error}（未连接 / 超时 / 连接断开已归一化）。
    """
