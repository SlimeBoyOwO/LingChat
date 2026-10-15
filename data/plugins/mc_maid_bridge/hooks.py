"""车万女仆桥接：把 MC 玩家的话送进对话，把角色的回话送回游戏内女仆气泡，
把游戏世界的变化记成旁白，并让角色能指挥女仆做几个动作。

四个入口共用本文件：`on_start`（启动/启用）、`on_ws`（WS 连接与收帧）、
`on_line_shown`（某句台词上屏）、`run`（LLM 调用本插件的工具）。

插件脚本每次调用都新建解释器、跑完即弃，所以这里不保存任何跨调用状态：
用得到的信息一律当场问（见 `_maid_id`），「自上次对话以来攒了几条事件」这类计数
也直接从台词表读回来（见 `_quiet_event_run`）。
"""

from plugin_host import (
    add_prompt_line,
    read_context,
    send_user_message,
    set_auto_mode,
    trigger_proactive,
    ws_open,
    ws_request,
    ws_send,
    ws_status,
)

CONN_ID = "tlm"
DECLARED_WS_URL = "ws://127.0.0.1:48920"

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

# 攒够几条游戏事件、且这期间玩家没说话，就请角色主动开口。0 = 只记旁白，不请。
DEFAULT_PROACTIVE_AFTER = 3

# 幂等检查先扫这么多条尾巴；不够长才整段再扫一次（见 `_entry_hint_written`）。
TAIL_SCAN = 50

# 等模组受理动作的回执。要明显小于工具自身的 15s 超时，否则模型在工具超时里干等。
ACTION_TIMEOUT_MS = 8000

# 工具名 → 模组认识的动作名。工具名带插件前缀，动作名不带。
_TOOL_ACTIONS = {
    "maid_go_to": "go_to",
    "maid_sit_down": "sit",
    "maid_stand_up": "stand",
    "maid_lie_down": "lie_down",
    "maid_look_at": "look_at",
}

# 带目标的动作与各自的默认目标。执行器只校验 JSON Schema 的一个子集、不认 enum，
# 所以合法值在这里兜底（模组那头还会再校验一次）。
_TARGET_DEFAULTS = {"go_to": "player", "lie_down": "bed"}

# 受理了但还没做完时给模型的话：必须让它知道自己还没到位，否则它会当场宣布已经办到。
_TOOL_PENDING_TEXT = {
    "go_to": "正在走过去，到了会告诉你。",
    "lie_down": "正在走过去，躺下了会告诉你。",
}

# 受理即完成的动作给模型的话。
_TOOL_DONE_TEXT = {
    "sit": "已经坐下了。",
    "stand": "已经站起来了。",
    "look_at": "已经看向玩家了。",
}

# 动作收尾时记进台词表的旁白。只陈述事实，情绪由模型自己决定。
_ACTION_DONE_TEXT = {
    "sit": "你坐下了。",
    "stand": "你站起来了。",
    "lie_down": "你躺到床上睡着了。",
    "look_at": "你转头看向了玩家。",
}

_DIMENSION_NAMES = {
    "minecraft:overworld": "主世界",
    "minecraft:the_nether": "下界",
    "minecraft:the_end": "末地",
}

# 常见生物群系的中文名。没收录的退回到 id 的后半段（英文），模型也认。
_BIOME_NAMES = {
    "minecraft:plains": "平原",
    "minecraft:sunflower_plains": "向日葵平原",
    "minecraft:forest": "森林",
    "minecraft:flower_forest": "繁花森林",
    "minecraft:birch_forest": "白桦林",
    "minecraft:dark_forest": "黑森林",
    "minecraft:taiga": "针叶林",
    "minecraft:snowy_taiga": "积雪针叶林",
    "minecraft:old_growth_pine_taiga": "原始松木针叶林",
    "minecraft:snowy_plains": "雪原",
    "minecraft:ice_spikes": "冰刺之地",
    "minecraft:desert": "沙漠",
    "minecraft:savanna": "热带草原",
    "minecraft:jungle": "丛林",
    "minecraft:sparse_jungle": "稀疏丛林",
    "minecraft:swamp": "沼泽",
    "minecraft:mangrove_swamp": "红树林沼泽",
    "minecraft:badlands": "恶地",
    "minecraft:beach": "沙滩",
    "minecraft:stony_shore": "石岸",
    "minecraft:river": "河流",
    "minecraft:ocean": "海洋",
    "minecraft:frozen_ocean": "冻洋",
    "minecraft:warm_ocean": "暖海",
    "minecraft:mushroom_fields": "蘑菇岛",
    "minecraft:windswept_hills": "风袭丘陵",
    "minecraft:meadow": "草甸",
    "minecraft:cherry_grove": "樱花树林",
    "minecraft:nether_wastes": "下界荒地",
    "minecraft:soul_sand_valley": "灵魂沙峡谷",
    "minecraft:crimson_forest": "绯红森林",
    "minecraft:warped_forest": "诡异森林",
    "minecraft:basalt_deltas": "玄武岩三角洲",
    "minecraft:the_end": "末地",
    "minecraft:end_highlands": "末地高地",
    "minecraft:small_end_islands": "末地小岛",
}

# 说不清是谁打的时候，用伤害类型兜个底（模组给的是伤害类型的 msg id）。
_DAMAGE_HINTS = {
    "fall": "从高处摔了下来",
    "lava": "碰到了岩浆",
    "in_fire": "被火烧到了",
    "on_fire": "身上着火了",
    "drown": "溺水了",
    "starve": "饿得不行",
    "cactus": "被仙人掌扎了",
    "freeze": "被冻伤了",
    "lightning_bolt": "被雷劈了",
    "magic": "被魔法击中",
    "wither": "中了凋零",
    "hot_floor": "踩到了岩浆块",
    "sweet_berry_bush": "被甜浆果丛扎了",
    "fly_into_wall": "撞到了墙上",
    "out_of_world": "掉进了虚空",
}


# ─── 入口 ───


def on_start(ctx):
    """启动与启用时各跑一次：把配置里的地址应用到声明的那条连接上。"""
    url = _config(ctx).get("ws_url") or ""
    url = url.strip() if isinstance(url, str) else ""
    # 声明里的 url 不能写占位符：玩家没保存过配置时 ctx.config 不含该键，
    # 占位符不会被替换。所以默认值写在 manifest，非默认值在这里覆盖。
    if url and url != DECLARED_WS_URL:
        ws_open(CONN_ID, url=url)


def on_ws(ctx):
    """WS 连接的建立与收帧。"""
    payload = ctx.get("payload") or {}
    if payload.get("connId") != CONN_ID:
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
    if not _truthy(_config(ctx).get("echo_to_game"), True):
        return
    if not _connected():
        return
    maid_id, _ = _maid_id(_config(ctx))
    if not maid_id:
        return
    ws_send(
        CONN_ID,
        {"type": "send_chat", "data": {"maid_id": maid_id, "message": message}},
    )


def run(ctx):
    """工具入口：把模型的动作意图翻成一条动作帧发给模组。

    返回的文本就是模型看到的结果，所以说实话最要紧：没连上、没绑定女仆、模组说
    办不到，一律如实回报，绝不假装已经执行——模型会照着这个结果跟玩家交代。
    """
    tool = (ctx.get("tool_name") or "").strip()
    action = _TOOL_ACTIONS.get(tool)
    if not action:
        return {"ok": False, "error": f"这个工具不存在：{tool}"}
    if not _connected():
        return {"ok": False, "error": "还没连上 Minecraft，做不到。"}
    maid_id, reason = _maid_id(_config(ctx))
    if not maid_id:
        return {"ok": False, "error": reason}

    data = {"maid_id": maid_id, "action": action}
    target = _action_target(action, ctx.get("args") or {})
    if target:
        data["target"] = target

    result = ws_request(
        CONN_ID, {"type": "maid_action", "data": data}, timeout_ms=ACTION_TIMEOUT_MS
    )
    if not result.get("ok"):
        return {"ok": False, "error": result.get("error") or "游戏没有回应"}

    frame = (result.get("response") or {}).get("data") or {}
    if not frame.get("accepted"):
        return {"ok": False, "error": frame.get("reason") or "游戏拒绝了这次动作"}
    if frame.get("async"):
        return {
            "ok": True,
            "message": _TOOL_PENDING_TEXT.get(action, "正在做，好了会告诉你。"),
        }
    return {"ok": True, "message": _TOOL_DONE_TEXT.get(action, "已经做完了。")}


def _action_target(action, args):
    """这次动作要带的目标；不带目标的动作返回空串。

    认不出的取值一律退回默认：执行器的 schema 校验不认 enum，模型可能塞别的字符串。
    """
    default = _TARGET_DEFAULTS.get(action)
    if not default:
        return ""
    raw = args.get("target")
    raw = raw.strip().lower() if isinstance(raw, str) else ""
    if raw not in ("player", "bed"):
        return default
    # 躺下只认床：player 这种取值对它没有意义
    if action == "lie_down" and raw != "bed":
        return default
    return raw


# ─── 下行：模组推来的帧 ───


def _bind_maid(ctx):
    """告诉模组盯哪只女仆。

    这一步是玩家说话能被推过来的前提：模组得先知道盯的是哪只女仆、且它的主人正是
    说话的那个玩家，才会把聊天转过来。不发这一条，MC 里的人说话永远没反应，而且
    是静默的——现象跟模组没装一模一样。

    每次重连都要重发：模组换存档或重启服务端时会把这个绑定清掉。
    """
    maid_id, reason = _maid_id(_config(ctx))
    if not maid_id:
        print(f"[mc_maid_bridge] 没能绑定女仆：{reason}")
        return
    ws_send(CONN_ID, {"type": "set_monitored_maid", "data": {"maid_id": maid_id}})


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


# ─── 下行：游戏事件 ───


def _handle_event(ctx, data):
    """一条游戏事件：记成旁白，该开口的时候请角色开口。

    紧急的（挨打、血量告急）当场开口；其余的攒够若干条、且这期间没人说话才请一次。
    """
    text, urgent = _format_event(data, _player_name(_config(ctx)))
    if not text:
        return
    written = add_prompt_line("narrator", EVENT_TAG + text)
    if not written.get("ok"):
        print(f"[mc_maid_bridge] 游戏事件没能写进台词表：{written.get('error')}")
        return
    if urgent or _quiet_event_run(written.get("index"), _proactive_after(ctx)):
        _nudge()


def _format_event(data, player_name=""):
    """模组推来的事件 → 一句只陈述事实的中文。返回 (文本, 是否紧急)。

    只写发生了什么，不写「该有什么情绪」——情绪由模型自己决定，留在台词表里的
    始终是客观记录。

    `player_name` 是玩家在 MC 里的用户名：事件里的名字撞上它就要改写成「玩家」，
    否则模型会当成不相干的第三个人。
    """
    kind = (data.get("event_type") or "").strip()
    if kind == "state_snapshot":
        return _snapshot_text(data), False
    if kind == "biome_change":
        return f"周围的环境变了，现在来到了{_name(_BIOME_NAMES, data.get('biome'))}。", False
    if kind == "dimension_change":
        came_from = _name(_DIMENSION_NAMES, data.get("from_dimension"))
        went_to = _name(_DIMENSION_NAMES, data.get("to_dimension"))
        return f"你跟着玩家从{came_from}来到了{went_to}。", False
    if kind == "time_phase_change":
        return ("天黑了。" if data.get("phase") == "night" else "天亮了。"), False
    if kind == "weather_change":
        if data.get("thundering"):
            return "变天了，外面在打雷。", False
        if data.get("raining"):
            return "开始下雨了。", False
        return "雨停了，天放晴了。", False
    if kind == "low_health":
        who = "你" if data.get("who") == "maid" else "玩家"
        now = _num(data.get("health"))
        most = _num(data.get("max_health"))
        return f"{who}的血量只剩 {now}/{most}，快撑不住了。", True
    if kind == "maid_hurt":
        return _hurt_text(data, player_name), True
    if kind == "action_done":
        # 动作的收尾是玩家（和模型）正在等的那件事，当场让角色交代一句
        return _action_done_text(data), True
    return "", False


def _action_done_text(data):
    """动作收尾。三种结局都要说：被顶掉也要说，否则角色会一直以为还在做。"""
    action = (data.get("action") or "").strip()
    status = (data.get("status") or "").strip()
    if status == "done":
        if action == "go_to":
            if (data.get("target") or "") == "bed":
                return "你走到了床边。"
            return "你走到了玩家身边。"
        return _ACTION_DONE_TEXT.get(action, "这件事你做完了。")
    if status == "cancelled":
        return "刚才那件事被新的事顶掉了，没做成。"
    reason = (data.get("reason") or "").strip()
    return f"刚才那件事没做成：{reason}" if reason else "刚才那件事没做成。"


def _snapshot_text(data):
    """接上世界时的第一帧：把眼下的处境交代清楚，模型才知道自己在哪儿。

    顺手把玩家在游戏里的名字写上：所有事件旁白里出现的都是这个名字，模型得先知道
    它指的就是玩家本人，才不会当成另一个人。名字来自模组，没配置也能说清。
    """
    player = (data.get("player_name") or "").strip()
    who = f"玩家（{player}）" if player else "玩家"
    clauses = [
        f"你正和{who}在"
        f"{_name(_DIMENSION_NAMES, data.get('dimension'))}的{_name(_BIOME_NAMES, data.get('biome'))}"
    ]
    # 下界 / 末地没有天空，也就没有昼夜与天气这回事
    if data.get("sky"):
        clauses.append("现在是白天" if data.get("phase") == "day" else "现在是夜里")
        clauses.append(_weather_phrase(data))
    health = f"你的血量 {_num(data.get('maid_health'))}/{_num(data.get('maid_max_health'))}"
    if _int(data.get("player_max_health"), 0) > 0:
        health += f"，玩家 {_num(data.get('player_health'))}/{_num(data.get('player_max_health'))}"
    return f"{'，'.join(clauses)}。{health}。"


def _weather_phrase(data):
    if data.get("thundering"):
        return "外面在打雷"
    if data.get("raining"):
        return "外面在下雨"
    return "天气晴朗"


def _hurt_text(data, player_name=""):
    """女仆受伤。模组会把一小段连续挨打合成一条，所以这里要照顾「几下」。"""
    attacker = (data.get("attacker") or "").strip()
    if attacker and player_name and attacker == player_name:
        cause = "被玩家打了"
    elif attacker:
        cause = f"被{attacker}打了"
    else:
        cause = _DAMAGE_HINTS.get((data.get("damage_type") or "").strip(), "挨了打")
    count = _int(data.get("count"), 1)
    times = f"连续 {count} 下，" if count > 1 else ""
    damage = _num(data.get("damage"))
    now = _num(data.get("health"))
    most = _num(data.get("max_health"))
    return f"你{cause}，{times}一共掉了 {damage} 点血，现在是 {now}/{most}。"


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


# ─── 进入 MC 的提示行 ───


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


# ─── 小工具 ───


def _config(ctx):
    return ctx.get("config") or {}


def _player_name(cfg):
    """玩家在 MC 里的用户名（配置项）。没填就是空串。

    只用来把事件旁白里撞上的名字改写成「玩家」——模组给的名字是裸的，模型不知道
    那是谁。填了最稳；不填也有一条兜底：接上游戏的第一帧旁白里会写明玩家叫什么。
    """
    raw = cfg.get("player_name")
    return raw.strip() if isinstance(raw, str) else ""


def _maid_id(cfg):
    """绑定的女仆 UUID；没配现问一次。返回 (id, 错误说明)。

    插件无状态存不下发现到的 id，所以未配置时每次用到都要重问一遍；
    本机回环，代价可忽略。填了配置就完全免掉这次往返。
    """
    configured = cfg.get("maid_id")
    configured = configured.strip() if isinstance(configured, str) else ""
    if configured:
        return configured, ""
    result = ws_request(CONN_ID, {"type": "get_maid_status", "data": {}}, timeout_ms=5000)
    if not result.get("ok"):
        return "", result.get("error") or "无法获取女仆状态"
    frame = result.get("response") or {}
    maids = ((frame.get("data") or {}).get("maids")) or []
    if not maids:
        return "", "游戏里没有找到女仆"
    return maids[0].get("id") or "", ""


def _connected():
    status = ws_status(conn_id=CONN_ID)
    return bool(status.get("ok")) and status.get("state") == "connected"


def _truthy(value, default):
    """配置项可能是布尔、也可能是字符串（前端表单传过字符串）。"""
    if value is None:
        return default
    if isinstance(value, str):
        return value.strip().lower() not in ("", "false", "0", "no")
    return bool(value)


def _proactive_after(ctx):
    """攒够几条游戏事件才请角色开口；0 = 只记旁白不请。"""
    raw = _config(ctx).get("proactive_after_events")
    if raw is None or raw == "":
        return DEFAULT_PROACTIVE_AFTER
    return max(0, _int(raw, DEFAULT_PROACTIVE_AFTER))


def _int(value, default=0):
    """配置与事件里的数字可能是浮点（前端表单、JSON）或字符串。"""
    try:
        return int(float(value))
    except (TypeError, ValueError):
        return default


def _num(value):
    """血量这类数字：整数不带小数点，其余留一位；给不出来就留个问号。"""
    try:
        number = float(value)
    except (TypeError, ValueError):
        return "?"
    if number == int(number):
        return str(int(number))
    return f"{number:.1f}"


def _name(names, raw):
    """游戏 id → 中文名；没收录的退回到 id 的后半段（英文），模型也认。"""
    identifier = (raw or "").strip()
    if not identifier:
        return "未知的地方"
    return names.get(identifier) or identifier.split(":")[-1].replace("_", " ")
