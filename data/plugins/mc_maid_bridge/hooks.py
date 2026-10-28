"""车万女仆桥接：把 MC 玩家的话送进对话，把角色的回话送回游戏内女仆气泡，
把游戏世界的变化记成旁白。

一个插件目录，几个文件各管一段（本文件是入口，manifest 的 `[[ws]]` / `[[subscribe]]` /
`[startup]` 都指向它）：

| 文件         | 管什么                                                     |
| ------------ | ---------------------------------------------------------- |
| `hooks.py`    | 这份：生命周期（启动 / 连接 / 某句上屏）与游戏事件的落地    |
| `bridge.py`   | 与模组的通道：连没连上、是哪只女仆、收发帧、读配置          |
| `wording.py`  | 注册表 id、枚举串、数字 → 中文，以及情绪 → 颜文字            |
| `naming.py`   | 旁白称呼：当前角色名与玩家名，以及模组回执的称呼替换        |
| `events/`     | 一类事件一个模块；紧急度、TTL、合并键与旁白登记在一起       |
| `tools/`      | 工具按行为分模块；`entry.py` 是 manifest 指向的入口         |
| `tools/context.py` | 她身边那一圈：快照与「某角色此刻」的措辞，工具与开口时共用 |

台词表里只留两种桥接写的东西：一类是事件旁白，另一类是进入 MC 时写一次的那条提示。
旁白一律用第三人称，角色与玩家都叫名字（见 `naming.py`），不写「你」「玩家」。
**感知信息只有一个入口：模组推事件帧，这里翻成旁白交给 `add_prompt_line`。** 不要在玩家
那句话后面用括号追加「（她此刻：…）」之类的处境；玩家说的话原样送进对话，她的姿势、周围
有什么、脚下踩着什么全部由事件旁白交代（见 `events/`）。

插件脚本每次调用都新建解释器、跑完即弃，所以这里不保存任何跨调用状态：
用得到的信息一律当场问（见 `bridge.ready`），「自上次对话以来攒了几条事件」这类计数
由宿主队列回给（见 `_quiet_event_run`）。
"""

from plugin_host import (
    add_prompt_line,
    read_context,
    read_data_file,
    send_user_message,
    set_auto_mode,
    set_reply_length_mode,
    set_voice_playback_available,
    trigger_proactive,
)

import bridge
from bridge import config, player_name, truthy
from events import event_key, event_severity, event_ttl, format_event, needs_snapshot
from naming import names
from tools.context import NEARBY_TIMEOUT_MS, creature_text, snapshot
from wording import (
    DEAD_STATE_MARK,
    RECOVERED_STATE_MARK,
    REVIVE_STATE_MARK,
    UNCONSCIOUS_STATE_MARK,
    kaomoji_for,
)


def _entry_hint():
    """进入 MC 的那条提示。名字是当下的角色名与玩家名，写法变了要重新交代一遍。

    幂等按内容判（见 `_entry_hint_written`），所以换角色、改用户名后会再写一条。
    """
    maid, player = names()
    return (
        f"［Minecraft 桥接］{maid}和{player}一起进入了 Minecraft 世界，"
        f"{maid}变成了游戏里的小女仆。\n"

        f"{maid}现在可以调用maid_开头的工具实现对游戏的操作啦！尽可能用 tool_search 找到的工具注册来帮助{player}！"
    )

# 幂等检查先扫这么多条尾巴；不够长才整段再扫一次（见 `_entry_hint_written`）。
TAIL_SCAN = 50

# 单句语音的上限。正常一句只有几百 KB，超过这个数多半是配置错了，别把整段
# base64 塞进一次 WS 帧。
MAX_TTS_BYTES = 8 * 1024 * 1024


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
        _send_favorite_food(ctx)
        _sync_game_voice()
        # 玩家这会儿在游戏里，腾不出手点对话框推进；不切自动模式的话，角色说完
        # 一句就停住，游戏里也就一直不冒下一句，看起来跟桥断了没两样。
        # 只负责开：断开时不还回去（用户要的就是这个语义）。
        set_auto_mode(True)
        # 游戏里的气泡装不下长篇大论；连上就切短档（1~2 句）。这一刻可能还没进入
        # 对话，设置会被拒，玩家从 MC 开口前还会再重申一次（见 _handle_chat）。
        _use_short_replies()
        _append_entry_hint()
    elif event == "message":
        _handle_frame(ctx, payload.get("data"))
    # disconnect / error 交给宿主的 auto_reconnect 退避重连，这里不做额外处理。


def on_line_shown(ctx):
    """某句台词上屏时触发（逐句）：同步游戏内气泡，按设置在游戏里播放这一句的语音。

    订阅的是「上屏」而不是「生成」：模型的回复是流式到达的，连着几句会几乎同时
    生成完。跟着上屏时机走，游戏里的女仆才会和主界面同一个节奏地冒话，而不是
    一次涌出去。

    模组只留两枚气泡：台词一枚、这一句情绪的颜文字一枚（候选在这边按情绪给，挑哪条
    由模组随机）。新的一句会替掉上一句的两枚，不会一句叠一句。

    气泡与语音是两个开关：关掉气泡仍然可以只要声音。
    """
    payload = ctx.get("payload") or {}
    # 剧本编辑器的预览流不该外发到游戏里。试玩代号可以是 0，必须按「键存在」判断。
    if payload.get("previewGen") is not None or not payload.get("roleId"):
        return
    # 每轮最后一句之后重念一次死亡/昏迷状态：这两种状态是持久的，模型几轮之后就会忘
    maid = ""
    if payload.get("isFinal"):
        _remind_state()
        # 这一轮说完了：动作收尾后按住的随机游荡可以放开（模组那边还有十秒兜底）
        maid, _ = bridge.ready(ctx)
        if maid:
            _settle(maid)
    cfg = config(ctx)
    message = (payload.get("message") or "").strip()
    audio_file = (payload.get("audioFile") or "").strip()
    want_bubble = bool(message) and truthy(cfg.get("echo_to_game"), True)
    want_voice = bool(audio_file) and truthy(cfg.get("play_voice_in_game"), False)
    if not want_bubble and not want_voice:
        return
    if not maid:
        maid, _ = bridge.ready(ctx)
    if not maid:
        return
    # 顺手重申一次监控目标：刚连上那一次可能没发出去（见 _bind_maid），而插件没有定时器、
    # 也存不下跨调用状态，「她说话」是唯一稳定的时机。模组那边幂等，重复发没有副作用。
    bridge.bind(maid, player_name(cfg))
    if want_bubble:
        # emotion 是分类器的预测标签（未启用时就是模型写的【情绪】），认不出的按「正常」给。
        emotion = (payload.get("emotion") or "").strip() or (
            payload.get("originalTag") or ""
        ).strip()
        bridge.send({
            "type": "send_chat",
            "data": {
                "maid_id": maid,
                "message": message,
                "kaomoji": kaomoji_for(emotion),
            },
        })
    if want_voice:
        _send_tts(maid, audio_file)


def _settle(maid):
    """告诉模组这一轮说完了：动作之后按住的随机游荡可以放开。

    模组不发这一帧也会在十秒后自己放开、主人走远时也会立刻放开，所以发不出去不算事。
    """
    bridge.send({"type": "maid_settle", "data": {"maid_id": maid}})


def _send_tts(maid, audio_file):
    """把这一句的音频原样发给模组，由它在女仆的位置播放（宿主此时已把本机静音）。

    读不到文件或发不出去都要记一笔：这时候本机是静音的，游戏里也没响就是彻底没声音，
    静默返回会让「语音没了」变成一个只能猜的问题。
    """
    if "/" in audio_file or "\\" in audio_file or audio_file in (".", ".."):
        return
    result = read_data_file(f"voice/{audio_file}")
    if not result.get("ok"):
        print(f"[mc_maid_bridge] 游戏内语音读取失败：{result.get('error')}")
        return
    if (result.get("size") or 0) > MAX_TTS_BYTES:
        print(f"[mc_maid_bridge] 游戏内语音过大，跳过：{result.get('size')} 字节")
        return
    audio = result.get("base64") or ""
    if not audio:
        return
    sent = bridge.send({
        "type": "play_tts",
        "data": {"maid_id": maid, "format": "wav", "audio": audio},
    })
    if not sent.get("ok"):
        print(f"[mc_maid_bridge] 游戏内语音没能发出去：{sent.get('error')}")


def _use_short_replies(quiet=False):
    """把回复台词数量切到短档（1~2 句）。

    连接那一刻 LingChat 可能还在主菜单、没进入对话，宿主会拒绝这次设置；所以玩家
    每次从 MC 开口、真正要生成回复之前还会再调一次（见 `_handle_chat`）。档位没变时
    宿主不写任何台词，重复调用是安全的。
    """
    result = set_reply_length_mode("short")
    if not result.get("ok") and not quiet:
        print(f"[mc_maid_bridge] 没能把回复长度切成短档：{result.get('error')}")


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
    result = bridge.bind(maid, player_name(bridge.config(ctx)))
    if not result.get("ok"):
        print(f"[mc_maid_bridge] 绑定帧没能发出去：{result.get('error')}")


def _send_favorite_food(ctx):
    """把她最喜欢吃的东西交给模组。

    这是角色的设定，所以填在软件这边的插件配置里；模组只存着照用，不设自己的配置项。
    插件没有定时器，就在连接时与玩家每次开口前各发一次：改完设置不用重连，下次在
    游戏里说话时生效。模组那边幂等，重复发没有副作用。
    """
    raw = config(ctx).get("favorite_food") or ""
    raw = raw if isinstance(raw, str) else ""
    items = [one.strip() for one in raw.replace("，", ",").split(",") if one.strip()]
    bridge.send({"type": "set_favorite_food", "data": {"items": items}})


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
    # 连接时可能还没进入对话，短档设置被拒；真正要生成回复前再重申一次。幂等，
    # 档位没变时不往台词表写任何行。
    _use_short_replies(quiet=True)
    # 顺手重申一次她最喜欢吃的东西：改完设置不用重连（见 _send_favorite_food）
    _send_favorite_food(ctx)
    # 与玩家在输入框里发一句走同一条路：写进台词历史、按当前配置生成回复，
    # 所以聊天窗里看得到，角色的回话也会顺着 on_line_shown 变成游戏内气泡。
    # 玩家这句话原样送，不在后面追加任何她的状态：感知一律走旁白入口（见模块说明）。
    send_user_message(text)


def _handle_event(ctx, data):
    """一条游戏事件：交给宿主排队，该开口的时候请角色开口。

    紧急的（事件模块里登记的 urgent 规则，外加「主人死在旁边」「捡到玩家丢的东西」
    两处按数据判定）当场开口；其余的攒够若干件、且这期间没人说话才请一次。

    事件一律带有效期交给宿主：它不会立刻进台词表，等角色真要开口那一刻再按有效期
    筛一遍，过期的直接丢掉——一场战斗里捡东西、挨打、怪物死掉会连着来，等第一轮
    台词念完，早先那些小事已经不值得再提了。
    """
    kind = (data.get("event_type") or "").strip()
    text, urgent = format_event(data)
    if not text:
        return
    if kind in ("maid_death", "maid_revive", "maid_unconscious", "maid_recovered"):
        # 死亡与昏迷时游戏里那句语音不该从她身上出来：改由本机放，醒来/复活后再交回去。
        set_voice_playback_available(kind in ("maid_revive", "maid_recovered"))
    if urgent and needs_snapshot(kind):
        # 出事这一刻正好是「再感知一次周围」的时机：怪物那条有 2 分钟防抖管着，
        # 这期间新来的谁都不会各记一条，所以她身边站着什么就跟着这条一起交代。
        text += _creature_text(data)
    pending = _append_event(text, kind, data)
    if pending is None:
        return
    if urgent:
        _nudge(urgent=True)
    elif _quiet_event_run(pending, bridge.proactive_after(ctx)):
        _nudge()


def _append_event(text, kind, data):
    """把一条事件交给宿主排队。返回队列里还有效的事件条数；写失败返回 None。

    `ttl_seconds` 让宿主在角色真要开口时才判「还值不值得说」；`key` + `severity`
    让连着挨打只留最重的一次。排队条数由宿主算好回给我们——插件没有跨调用状态，
    「攒够几条了」只能问队列。
    """
    written = add_prompt_line(
        "narrator",
        text,
        ttl_seconds=event_ttl(kind, data),
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
    data, _ = snapshot(maid, NEARBY_TIMEOUT_MS)
    return creature_text(data) if data else ""


# 死亡状态问模组时的超时：本机回环，正常是毫秒级。游戏暂停时模组不推进队列，
# 超时了就退回台词表判断，不让一次查询拖住这一句。
MAID_STATE_TIMEOUT_MS = 800

# 台词表里往回扫多少条找死亡/复活旁白。
LIFE_SCAN = 200


def _remind_state():
    """她死了或昏迷着，就每一轮再念一次状态。

    两种状态都是持久的，模型几轮之后就会把她当成还站在旁边的小女仆。判断先问模组
    （它记着这一局的死活与昏迷），问不到才退回台词表里最后一条状态旁白——历史本身
    就是状态。带上 key 让同一轮里几句台词只留一条。
    """
    state = _state_now()
    maid, _ = names()
    if state.get("dead"):
        result = add_prompt_line(
            "narrator",
            f"{maid}{DEAD_STATE_MARK}，在复活之前没法行动，也看不到听不到游戏里的事。",
            ttl_seconds=604800,
            key="maid_dead_state",
            severity=1,
        )
        if not result.get("ok"):
            print(f"[mc_maid_bridge] 死亡状态没能进队列：{result.get('error')}")
        return
    if not state.get("unconscious"):
        return
    if state.get("on_bed"):
        text = (
            f"{maid}{UNCONSCIOUS_STATE_MARK}，正躺在床上慢慢恢复，身边冒着爱心，"
            "还要等一会儿才会醒。"
        )
    else:
        text = (
            f"{maid}{UNCONSCIOUS_STATE_MARK}，倒在地上叫不醒；"
            f"把{maid}抱到床上（普通床或女仆床）连续躺一分钟，{maid}才会醒。"
        )
    result = add_prompt_line(
        "narrator",
        text,
        ttl_seconds=604800,
        key="maid_unconscious_state",
        severity=1,
    )
    if not result.get("ok"):
        print(f"[mc_maid_bridge] 昏迷状态没能进队列：{result.get('error')}")


def _state_now():
    """她现在是不是死的、是不是昏迷着。模组这一局没记录时退回台词表判断。"""
    response, error = bridge.ask({"type": "maid_state", "data": {}}, MAID_STATE_TIMEOUT_MS)
    if not error and response.get("type") == "maid_state_result":
        data = response.get("data") or {}
        if data.get("known"):
            return {
                "dead": bool(data.get("dead")),
                "unconscious": bool(data.get("unconscious")),
                "on_bed": bool(data.get("on_bed")),
            }
    state = _life_state()
    return {
        "dead": state == "dead",
        "unconscious": state == "unconscious",
        "on_bed": False,
    }


def _sync_game_voice():
    """连上时对一次账：她若还死在上一局或昏迷着，语音就该由本机放。

    宿主不知道自己那边有没有女仆，只会按「开关开着 + 连接在」把本机静音；
    死了的她在游戏里没有实体，昏迷的她不该从身上发声。之后她死、复活、昏迷、醒来时
    再各报一次（见 _handle_event）。
    """
    state = _state_now()
    set_voice_playback_available(not state.get("dead") and not state.get("unconscious"))


def _life_state():
    """台词表末尾最后一条状态旁白说的是什么：dead / unconscious / alive / unknown。"""
    head = read_context(start=1, end=1)
    total = head.get("total") or 0
    if not head.get("ok") or not total:
        return "unknown"
    since = max(1, total - LIFE_SCAN)
    lines = read_context(start=since, end=total).get("lines") or []
    state = "unknown"
    for line in lines:
        content = line.get("content") or ""
        if DEAD_STATE_MARK in content:
            state = "dead"
        elif REVIVE_STATE_MARK in content:
            state = "alive"
        elif UNCONSCIOUS_STATE_MARK in content:
            state = "unconscious"
        elif RECOVERED_STATE_MARK in content:
            state = "alive"
    return state


def _quiet_event_run(pending, count):
    """排队中的事件刚好攒够 count 件时返回真。

    用户要的「状态更新到一定量、且这期间玩家没说话」就是这个：角色一旦开口，宿主
    就把队列写进台词表并清空，计数自然归零——所以插件不必自己存计数器。带有效期的
    条目在入队时已经筛过一遍，这里的数只算还活着的。

    用「刚好到量」而不是「至少」：到了量就请过一次口，之后同一个队列里不再重复催。
    """
    return count > 0 and pending == count


def _nudge(urgent=False):
    """请宿主让角色主动开一次口。

    空提示语：该说的东西已经作为旁白记在台词表里了，再补一句「你注意到身边有变化、
    主动说一句」既替角色决定了该看什么，也容易把语气带成复述报告。宿主收到空提示
    就直接接着上下文生成一轮，不往台词表写任何行。

    `urgent` 只给死亡这类事：宿主会顶掉排队中的普通请求，投放时也不吃冷却与
    上一轮的静默期。

    可能被拒（主动对话没开、用户正在输入、上一轮还没说完），那就下次再说：
    事件已经排在宿主队列里，有效期之内下一次开口仍会带上。
    """
    result = trigger_proactive("", urgent=urgent)
    if result.get("ok") and not result.get("triggered"):
        print(f"[mc_maid_bridge] 角色没能主动开口：{result.get('reason')}")


def _append_entry_hint():
    """往台词表末尾追加一条「进入 MC」的旁白提示，幂等。

    走宿主原生的 `add_prompt_line`：它按引擎自己的写法落行（属性 User、不带发言人、
    内容包成 `{旁白: …}`），所以模型知道这是提示而不是玩家说的话，也不会被算进
    玩家消息序号与「真实对话」。
    """
    hint = _entry_hint()
    if _entry_hint_written(hint):
        return
    result = add_prompt_line("narrator", hint)
    if not result.get("ok"):
        print(f"[mc_maid_bridge] 进入 MC 的提示行没能写入：{result.get('error')}")


def _entry_hint_written(hint):
    """台词表里是否已经写过**现在这一版**的提示行。

    按内容认，不按标记认：措辞改了以后老存档也要重新交代一遍，否则模型一直按旧
    规矩理解事件旁白。先看尾巴（重连的常见情形，便宜）；历史
    比尾巴窗口更长、尾巴里又没找到时，才整段扫一遍确认。
    """
    head = read_context(start=1, end=1)
    total = head.get("total") or 0
    if not head.get("ok") or not total:
        return False
    since = max(1, total - TAIL_SCAN)
    if _has_hint(read_context(start=since, end=total), hint):
        return True
    if since <= 1:
        return False
    return _has_hint(read_context(), hint)


def _has_hint(history, hint):
    lines = history.get("lines") or []
    return any(hint in (line.get("content") or "") for line in lines)
