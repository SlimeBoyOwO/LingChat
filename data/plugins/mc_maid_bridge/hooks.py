"""车万女仆桥接：把 MC 玩家的话送进对话，把角色的回话送回游戏内女仆气泡，
把游戏世界的变化记成旁白。

一个插件目录，几个文件各管一段（本文件是入口，manifest 的 `[[ws]]` / `[[subscribe]]` /
`[startup]` 都指向它）：

| 文件         | 管什么                                                     |
| ------------ | ---------------------------------------------------------- |
| `hooks.py`   | 这份：生命周期（启动 / 连接 / 某句上屏）与游戏事件的落地    |
| `bridge.py`  | 与模组的通道：连没连上、是哪只女仆、收发帧、读配置          |
| `wording.py` | 注册表 id、枚举串、数字 → 中文                              |
| `events.py`  | 一类事件 → 一句旁白（加一类事件就在这里登记）               |
| `tools.py`   | 六个工具的入口（manifest 的 `[[tools]]` 指向它）            |

插件脚本每次调用都新建解释器、跑完即弃，所以这里不保存任何跨调用状态：
用得到的信息一律当场问（见 `bridge.ready`），「自上次对话以来攒了几条事件」这类计数
也直接从台词表读回来（见 `_quiet_event_run`）。
"""

from plugin_host import (
    add_prompt_line,
    read_context,
    send_user_message,
    set_auto_mode,
    trigger_proactive,
)

import bridge
from bridge import config, player_name, truthy
from events import format_event

# 提示行的标记前缀，用来在台词表里认出自己写过的那条——幂等就靠它。
ENTRY_TAG = "［Minecraft 桥接］"

# 游戏事件的标记前缀。既用来认自己写过的行，也用来判「自上次对话以来攒了几条」。
EVENT_TAG = "［MC 事件］"

ENTRY_HINT = (
    "［Minecraft 桥接］你已和玩家一起进入 Minecraft 世界，你变成了游戏里的小女仆。\n"
    "- 你在 LingChat 回的话会同步显示成游戏内女仆的聊天气泡，照常说话就行。\n"
    "- 以「［Minecraft］」开头的消息来自游戏世界，是玩家在 MC 里说的，按玩家发言对待。\n"
    "- 以「［MC 事件］」开头的是世界本身的旁白（换地形、天黑、下雨、受伤这类），"
    "它不是谁说的话，当背景信息看。\n"
    "- 玩家让你做动作（过来、坐下、看着我、去床上躺下）时用 maid_ 开头的工具指挥女仆："
    "她不会瞬移，工具说「正在过去」就是还没到，等旁白说她到了再当这件事办成了。"
)

# 幂等检查先扫这么多条尾巴；不够长才整段再扫一次（见 `_entry_hint_written`）。
TAIL_SCAN = 50


def on_start(ctx):
    """启动与启用时各跑一次：把配置里的地址应用到声明的那条连接上。

    声明里的 url 不能写占位符：玩家没保存过配置时 ctx.config 不含该键，占位符不会被
    替换。所以默认值写在 manifest，非默认值在这里覆盖。
    """
    url = config(ctx).get("ws_url") or ""
    url = url.strip() if isinstance(url, str) else ""
    if url and url != bridge.DECLARED_WS_URL:
        bridge.open_connection(url)


def on_ws(ctx):
    """WS 连接的建立与收帧。"""
    payload = ctx.get("payload") or {}
    if payload.get("connId") != bridge.CONN_ID:
        return
    event = payload.get("event")
    if event == "connect":
        _bind_maid(ctx)
        # 玩家这会儿在游戏里，腾不出手点对话框推进；不切自动模式的话，角色说完
        # 一句就停住，游戏里也就一直不冒下一句，看起来跟桥断了没两样。
        # 只负责开：断开时不还回去（用户要的就是这个语义）。
        set_auto_mode(True)
        _append_entry_hint()
    elif event == "message":
        _handle_frame(ctx, payload.get("data"))
    # disconnect / error 交给宿主的 auto_reconnect 退避重连，这里不做额外处理。


def on_line_shown(ctx):
    """某句台词上屏时触发（逐句）：把这一句同步成游戏内气泡。

    订阅的是「上屏」而不是「生成」：模型的回复是流式到达的，连着几句会几乎同时
    生成完。跟着上屏时机走，游戏里的女仆才会和主界面同一个节奏地冒话，而不是
    一次涌出去。
    """
    payload = ctx.get("payload") or {}
    # 剧本编辑器的预览流不该外发到游戏里。
    if payload.get("previewGen") or not payload.get("roleId"):
        return
    message = (payload.get("message") or "").strip()
    if not message:
        return
    if not truthy(config(ctx).get("echo_to_game"), True):
        return
    maid, _ = bridge.ready(ctx)
    if not maid:
        return
    bridge.send({"type": "send_chat", "data": {"maid_id": maid, "message": message}})


def _bind_maid(ctx):
    """告诉模组盯哪只女仆。

    这一步是玩家说话能被推过来的前提：模组得先知道盯的是哪只女仆、且它的主人正是
    说话的那个玩家，才会把聊天转过来。不发这一条，MC 里的人说话永远没反应，而且
    是静默的——现象跟模组没装一模一样。

    每次重连都要重发：模组换存档或重启服务端时会把这个绑定清掉。
    """
    maid, reason = bridge.ready(ctx)
    if not maid:
        print(f"[mc_maid_bridge] 没能绑定女仆：{reason}")
        return
    bridge.send({"type": "set_monitored_maid", "data": {"maid_id": maid}})


def _handle_frame(ctx, data):
    """模组推来的一帧。目前两类：玩家说的话、游戏世界的事件。"""
    if not isinstance(data, dict):
        return
    frame_type = data.get("type")
    if frame_type == "chat_message":
        _handle_chat(data.get("data") or {})
    elif frame_type == "event":
        _handle_event(ctx, data.get("data") or {})


def _handle_chat(chat):
    message = (chat.get("message") or "").strip()
    if not message:
        return
    # 发言者恒为玩家本人：模组只在「说话的人正是这只女仆的主人」时才转发，uuid 对不上
    # 直接丢。所以用户名不能原样写进去——模型会当成第三个人在说话，而不是玩家。
    sender = (chat.get("sender") or "").strip()
    who = f"玩家（{sender}）" if sender else "玩家"
    # 与玩家在输入框里发一句走同一条路：写进台词历史、按当前配置生成回复，
    # 所以聊天窗里看得到，角色的回话也会顺着 on_line_shown 变成游戏内气泡。
    send_user_message(f"［Minecraft］{who}说：{message}")


def _handle_event(ctx, data):
    """一条游戏事件：记成旁白，该开口的时候请角色开口。

    紧急的（挨打、血量告急、敌对生物靠近、动作收尾）当场开口；其余的攒够若干条、
    且这期间没人说话才请一次。
    """
    text, urgent = format_event(data, player_name(config(ctx)))
    if not text:
        return
    written = add_prompt_line("narrator", EVENT_TAG + text)
    if not written.get("ok"):
        print(f"[mc_maid_bridge] 游戏事件没能写进台词表：{written.get('error')}")
        return
    if urgent or _quiet_event_run(written.get("index"), bridge.proactive_after(ctx)):
        _nudge()


def _quiet_event_run(index, count):
    """最近 count 行是否清一色是自己写的游戏事件行。

    用户要的「状态更新到一定量、且这期间玩家没说话」就是这个：玩家发言与角色的
    回话都会插进这个连续段、把它打断，计数于是自然归零——所以插件不必自己存计数器。
    """
    if not index or count <= 0:
        return False
    window = read_context(start=max(1, index - count + 1), end=index)
    lines = window.get("lines") or []
    if len(lines) < count:
        return False
    return all(EVENT_TAG in (line.get("content") or "") for line in lines)


def _nudge():
    """请宿主让角色主动开一次口。

    空提示语：该说的东西已经作为旁白记在台词表里了，再补一句「你注意到身边有变化、
    主动说一句」既替角色决定了该看什么，也容易把语气带成复述报告。宿主收到空提示
    就直接接着上下文生成一轮，不往台词表写任何行。

    可能被拒（主动对话没开、用户正在输入、上一轮还没说完），那就下次再说：
    事件行已经记在台词表里，上下文不缺这一段。
    """
    result = trigger_proactive("")
    if result.get("ok") and not result.get("triggered"):
        print(f"[mc_maid_bridge] 角色没能主动开口：{result.get('reason')}")


def _append_entry_hint():
    """往台词表末尾追加一条「进入 MC」的旁白提示，幂等。

    走宿主原生的 `add_prompt_line`：它按引擎自己的写法落行（属性 User、不带发言人、
    内容包成 `{旁白: …}`），所以模型知道这是提示而不是玩家说的话，也不会被算进
    玩家消息序号与「真实对话」。
    """
    if _entry_hint_written():
        return
    result = add_prompt_line("narrator", ENTRY_HINT)
    if not result.get("ok"):
        print(f"[mc_maid_bridge] 进入 MC 的提示行没能写入：{result.get('error')}")


def _entry_hint_written():
    """台词表里是否已经有过提示行。

    先看尾巴（重连的常见情形，便宜）；历史比尾巴窗口更长、尾巴里又没找到时，
    才整段扫一遍确认——避免历史很长时每次连接都做一次全量导出。
    """
    head = read_context(start=1, end=1)
    total = head.get("total") or 0
    if not head.get("ok") or not total:
        return False
    since = max(1, total - TAIL_SCAN)
    if _has_entry_tag(read_context(start=since, end=total)):
        return True
    if since <= 1:
        return False
    return _has_entry_tag(read_context())


def _has_entry_tag(history):
    lines = history.get("lines") or []
    return any(ENTRY_TAG in (line.get("content") or "") for line in lines)
