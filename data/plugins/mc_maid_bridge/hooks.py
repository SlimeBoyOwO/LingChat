"""车万女仆桥接：把 MC 玩家的话送进对话，把角色的回话送回游戏内女仆气泡，
把游戏世界的变化记成旁白。

一个插件目录，几个文件各管一段（本文件是入口，manifest 的 `[[ws]]` / `[[subscribe]]` /
`[startup]` 都指向它）：

| 文件         | 管什么                                                     |
| ------------ | ---------------------------------------------------------- |
| `hooks.py`   | 这份：生命周期（启动 / 连接 / 某句上屏）与游戏事件的落地    |
| `bridge.py`  | 与模组的通道：连没连上、是哪只女仆、收发帧、读配置          |
| `wording.py` | 注册表 id、枚举串、数字 → 中文，以及情绪 → 颜文字            |
| `events.py`  | 一类事件 → 一句旁白（加一类事件就在这里登记）               |
| `tools.py`   | 六个工具的入口，外加她身边那一圈的措辞（工具与开口时携带）  |

台词表里只留两种桥接写的东西：一类是 `［MC 事件］` 开场的事件旁白，另一类是进入 MC
时写一次的那条提示。她身边那一圈不进台词表——那些事由玩家开口那一刻随消息带过去
（见 `_carry`），逐条记下来会把历史塞满同类句子。

插件脚本每次调用都新建解释器、跑完即弃，所以这里不保存任何跨调用状态：
用得到的信息一律当场问（见 `bridge.ready`），「自上次对话以来攒了几条事件」这类计数
由宿主队列回给（见 `_quiet_event_run`）。
"""

from plugin_host import (
    add_prompt_line,
    read_context,
    send_user_message,
    set_auto_mode,
    trigger_proactive,
)

import bridge
import tools
from bridge import config, player_name, truthy
from events import event_key, event_severity, event_ttl, format_event
from wording import kaomoji_for

# 游戏事件的标记前缀。模型与人都靠它认出这是世界记录而不是谁说的话。
EVENT_TAG = "［MC 事件］"

# 出事那一刻顺手再问一次「她周围有什么」的几类。收到东西、被抱起来、拿到成就都不是
# 出事，不必多一次往返；被抱着的时候她根本不在世界里，问了也取不到。
SNAPSHOT_EVENTS = frozenset({"maid_hurt", "low_health", "threat_near", "action_done", "creature_death"})

# 进入 MC 的那条提示。开头是它的标记（模型与人都靠它认出这是桥接写的），写法变了要
# 重新交代一遍，幂等按内容判，见 `_entry_hint_written`。
ENTRY_HINT = (
    "［Minecraft 桥接］你已和玩家一起进入 Minecraft 世界，你变成了游戏里的小女仆。\n"
    "- 你在 LingChat 回的话会同步显示成游戏内女仆的聊天气泡，照常说话就行。\n"
    "- 玩家在游戏里打出的话会当成玩家自己的发言送过来，和在输入框里发的一句一样。\n"
    "- 玩家那句话后面若还跟着一句「（你此刻：…）」，那是你当时看到的情形（脚下踩着哪、"
    "头顶遮没遮住、玩家在不在你身边、身边有什么活物），不是玩家说的话。\n"
    "- 以「［MC 事件］」开头的是世界本身的记录（换地形、天黑、下雨、受伤、有生物死亡、"
    "玩家丢给你或喂你东西、玩家把你抱起来或放下、玩家拿到成就这类），它不是谁说的话，"
    "当背景信息看；几件事接连发生时可能记在同一行里，用换行隔开。\n"
    "- 玩家往你背包里放东西、丢东西给你、拿食物潜行右键喂你，都会记成一条旁白；"
    "旁白里说收到或捡到的东西，就是已经在你身边或背包里的。\n"
    "- 玩家让你做动作（过来、坐下、站起来、去床上躺下）时用 maid_ 开头的工具指挥女仆："
    "她不会瞬移，工具回执只说明指令已收到，不代表已经到位。\n"
    "- 玩家叫你干活（种地、钓鱼、打怪）或者叫你别干了，用 maid_set_task 换工作模式；"
    "她按自己的作息干活，切换成功不等于她此刻已经开工，工具结果里会写明。\n"
    "- 玩家叫你打谁（「打那只僵尸」「打我一下」）用 maid_attack 点名：活物的名字、"
    "hostile / animal，或者 owner（主人自己）。追着打是默认，会把目标往死里打；"
    "只是逗她抬手（「打我一下」「轻轻碰我一下」）要加 style=light，只打一下就收手。"
    "攻击回执只代表她开始打了，不代表目标已经倒下；在那之前只能说「正在打」。\n"
    "- 玩家叫你挖方块（「把这块挖了」「挖十块石头」）用 maid_mine：target 填方块名，"
    "不给就是玩家正看着的那块；count 填要挖几块。她会自己走过去、从背包里换合适的工具。"
    "挖掘回执只代表开始挖了，方块还没被挖掉。"
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

    模组只留两枚气泡：台词一枚、这一句情绪的颜文字一枚（候选在这边按情绪给，挑哪条
    由模组随机）。新的一句会替掉上一句的两枚，不会一句叠一句。
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
    # 顺手重申一次监控目标：刚连上那一次可能没发出去（见 _bind_maid），而插件没有定时器、
    # 也存不下跨调用状态，「她说话」是唯一稳定的时机。模组那边幂等，重复发没有副作用。
    bridge.bind(maid)
    # emotion 是分类器的预测标签（未启用时就是模型写的【情绪】），认不出的按「正常」给。
    emotion = (payload.get("emotion") or "").strip() or (payload.get("originalTag") or "").strip()
    bridge.send({
        "type": "send_chat",
        "data": {
            "maid_id": maid,
            "message": message,
            "kaomoji": kaomoji_for(emotion),
        },
    })


def _bind_maid(ctx):
    """告诉模组盯哪只女仆。

    这一步是玩家说话能被推过来的前提：模组得先知道盯的是哪只女仆、且它的主人正是
    说话的那个玩家，才会把聊天转过来。不发这一条，MC 里的人说话永远没反应，而且
    是静默的——现象跟模组没装一模一样。

    **这里不查连接状态**：这一条正是在「刚连上」的那一刻发的，而那一刻宿主自己对这个
    连接的登记可能还没落定（2026-10-16 真机上踩过：先问一句「连上了吗」得到否，于是
    绑定被静静跳过，聊天与感知一起哑掉，只剩「她的话能上屏」这一半还通）。直接问女仆
    是谁、再把绑定发出去，发不出去就记一笔；下一次上屏还会再说一遍（见 on_line_shown）。
    """
    maid, reason = bridge.maid_id(bridge.config(ctx))
    if not maid:
        print(f"[mc_maid_bridge] 没能绑定女仆：{reason}")
        return
    result = bridge.bind(maid)
    if not result.get("ok"):
        print(f"[mc_maid_bridge] 绑定帧没能发出去：{result.get('error')}")


def _handle_frame(ctx, data):
    """模组推来的一帧。目前两类：玩家说的话、游戏世界的事件。"""
    if not isinstance(data, dict):
        return
    frame_type = data.get("type")
    if frame_type == "chat_message":
        _handle_chat(ctx, data.get("data") or {})
    elif frame_type == "event":
        _handle_event(ctx, data.get("data") or {})


def _handle_chat(ctx, chat):
    message = (chat.get("message") or "").strip()
    if not message:
        return
    # 发言者恒为玩家本人：模组只在「说话的人正是这只女仆的主人」时才转发，uuid 对不上
    # 直接丢。所以这里原样送消息，连标记都不加——宿主会按玩家行补上 LingChat 设置里的
    # 用户名（「用户酱: …」），和输入框里发的一句完全同形。多写一个 MC 账号名或一个
    # 「［Minecraft］」都会凭空多出「第三个人」（2026-10-16 真机上模型照着那个形状，
    # 把玩家没说过的话写进了自己的台词）。配置里填了 MC 用户名、而说话的不是他时
    # （多人服里女仆的主人另有其人）才点名。
    sender = (chat.get("sender") or "").strip()
    cfg = config(ctx)
    who = player_name(cfg)
    if who and sender and sender != who:
        text = f"{sender}说：{message}"
    else:
        text = message
    note = _carry(chat)
    # 与玩家在输入框里发一句走同一条路：写进台词历史、按当前配置生成回复，
    # 所以聊天窗里看得到，角色的回话也会顺着 on_line_shown 变成游戏内气泡。
    send_user_message(f"{text}\n{note}" if note else text)


def _carry(chat):
    """随这次发言带一句她此刻的处境。

    身边那一圈（附近有什么活物、玩家进出视线、头顶遮挡、脚下踩到什么）不再逐条写成
    旁白——一整晚下来能把台词表塞满，而她要的只是「知道周围有什么」。改成开口这一刻
    现取一份快照带过去：一次发言带一次，说完就随那行留在历史里。

    女仆 id 直接用帧里的：模组只在「说话的人正是这只女仆的主人」时才转发，帧里必然带着
    她的 uuid，不必再问一次游戏。取不到就这一轮不带，玩家的话照常送。
    """
    maid = (chat.get("maid_id") or "").strip()
    if not maid:
        return ""
    data, _ = tools.snapshot(maid, tools.CARRY_TIMEOUT_MS)
    return tools.carry_text(data) if data else ""


def _handle_event(ctx, data):
    """一条游戏事件：交给宿主排队，该开口的时候请角色开口。

    紧急的（`events.URGENT`，外加「主人死在旁边」「捡到玩家丢的东西」两处特判）当场
    开口；其余的攒够若干件、且这期间没人说话才请一次。

    事件一律带有效期交给宿主：它不会立刻进台词表，等角色真要开口那一刻再按有效期
    筛一遍，过期的直接丢掉——一场战斗里捡东西、挨打、怪物死掉会连着来，等第一轮
    台词念完，早先那些小事已经不值得再提了。
    """
    kind = (data.get("event_type") or "").strip()
    text, urgent = format_event(data)
    if not text:
        return
    if urgent and kind in SNAPSHOT_EVENTS:
        # 出事这一刻正好是「再感知一次周围」的时机：怪物那条有 2 分钟防抖管着，
        # 这期间新来的谁都不会各记一条，所以她身边站着什么就跟着这条一起交代。
        text += _creature_text(data)
    pending = _append_event(text, kind, data)
    if pending is None:
        return
    if urgent or _quiet_event_run(pending, bridge.proactive_after(ctx)):
        _nudge()


def _append_event(text, kind, data):
    """把一条事件交给宿主排队。返回队列里还有效的事件条数；写失败返回 None。

    `ttl_seconds` 让宿主在角色真要开口时才判「还值不值得说」；`key` + `severity`
    让连着挨打只留最重的一次。排队条数由宿主算好回给我们——插件没有跨调用状态，
    「攒够几条了」只能问队列。
    """
    written = add_prompt_line(
        "narrator",
        EVENT_TAG + text,
        ttl_seconds=event_ttl(kind),
        key=event_key(kind),
        severity=event_severity(kind, data),
    )
    if not written.get("ok"):
        print(f"[mc_maid_bridge] 游戏事件没能进队列：{written.get('error')}")
        return None
    return int(written.get("pending") or 0)


def _creature_text(data):
    """她身边有什么活物，一句话（没有就空串）。取不到就这一条不带。"""
    maid = (data.get("maid_id") or "").strip()
    if not maid:
        return ""
    snapshot, _ = tools.snapshot(maid, tools.CARRY_TIMEOUT_MS)
    return tools.creature_text(snapshot) if snapshot else ""


def _quiet_event_run(pending, count):
    """排队中的事件刚好攒够 count 件时返回真。

    用户要的「状态更新到一定量、且这期间玩家没说话」就是这个：角色一旦开口，宿主
    就把队列写进台词表并清空，计数自然归零——所以插件不必自己存计数器。带有效期的
    条目在入队时已经筛过一遍，这里的数只算还活着的。

    用「刚好到量」而不是「至少」：到了量就请过一次口，之后同一个队列里不再重复催。
    """
    return count > 0 and pending == count


def _nudge():
    """请宿主让角色主动开一次口。

    空提示语：该说的东西已经作为旁白记在台词表里了，再补一句「你注意到身边有变化、
    主动说一句」既替角色决定了该看什么，也容易把语气带成复述报告。宿主收到空提示
    就直接接着上下文生成一轮，不往台词表写任何行。

    可能被拒（主动对话没开、用户正在输入、上一轮还没说完），那就下次再说：
    事件已经排在宿主队列里，有效期之内下一次开口仍会带上。
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
    """台词表里是否已经写过**现在这一版**的提示行。

    按内容认，不按那个标记认：措辞改了以后老存档也要重新交代一遍，否则模型一直按旧
    规矩理解「［MC 事件］」与「（你此刻：…）」。先看尾巴（重连的常见情形，便宜）；历史
    比尾巴窗口更长、尾巴里又没找到时，才整段扫一遍确认。
    """
    head = read_context(start=1, end=1)
    total = head.get("total") or 0
    if not head.get("ok") or not total:
        return False
    since = max(1, total - TAIL_SCAN)
    if _has_hint(read_context(start=since, end=total)):
        return True
    if since <= 1:
        return False
    return _has_hint(read_context())


def _has_hint(history):
    lines = history.get("lines") or []
    return any(ENTRY_HINT in (line.get("content") or "") for line in lines)
