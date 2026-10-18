"""十个工具的入口（manifest 的 `[[tools]]` 指向本文件），外加「她身边那一圈」的措辞。

`run(ctx)` 返回的文本就是模型看到的结果，所以说实话最要紧：没连上、没绑定女仆、模组说
办不到，一律如实回报，绝不假装已经执行——模型会照着这个结果跟玩家交代。

工具分六类：
- 动作（走出去、坐下、站起、躺下）：把意图翻成一条 `maid_action` 帧，受理与否
  当场给答复；走过去这类耗时的，结果稍后由 `action_done` 事件旁白交代。
- 工作模式（换她接下来去干什么活）：翻成一条 `maid_set_task` 帧，切的就是她自己界面里
  那个任务，成败与「现在会不会真去干」一并回给模型。
- 攻击（打她身边的哪个活物）：翻成一条 `maid_attack` 帧。点谁由玩家说了算，连主人自己
  都能被点名；她会先切到攻击模式，没有武器就如实失败。
- 挖掘（挖玩家看着的那块、或附近的某一种方块）：翻成一条 `maid_mine` 帧。受理只代表
  开始挖了；挖穿、挖够、走不到都由 `maid_mine_done` 事件旁白交代。
- 交物品（把她背包里的东西交给玩家）：同步完成，给了几件、她身上有没有，当场回给模型。
- 查询（看一眼周围）：现取一份快照翻成几句事实，只回给模型看，不写台词表。
- 看图（看一眼玩家屏幕）：抓一张画面交给宿主，宿主把它当图片附给模型——她这才看得到
  你搭的爱心是什么形状。

「看向玩家」不是工具：她每说一句话，模组自己会转头看主人（打架、干活、睡觉时除外）。

那份快照有第二个用处：玩家在 MC 里说话时，随那句话带一句「你此刻的处境」（见
`carry_text`）；她挨打那类「出事」的旁白后面也缀一句眼前的活物（见 `creature_text`）。
身边的变化不再逐条写成旁白，她知道周围有什么就靠这两处。

给模型的话一律不带数字：格数、血量、光照写进上下文，它就会照本宣科地复述
「大约 10.5 格」这种句子，而玩家并不需要知道几位小数。
"""

from plugin_host import capture_screen

from bridge import ask, config, ready
from wording import (
    BLOCK_NAMES,
    DIMENSION_NAMES,
    MOB_NAMES,
    as_int,
    attack_style,
    attack_target,
    heard_sounds,
    item_choices,
    item_name,
    item_target,
    name_of,
    mine_choices,
    mine_target,
    nearness_place,
    sighting,
    task_choices,
    task_id,
    task_name,
)

# 动作的回执要等模组受理，超时明显小于工具自身的 15s，否则模型在工具超时里干等。
ACTION_TIMEOUT_MS = 8000
# 看一眼周围等快照的超时。单人游戏按 Esc 暂停时它必然超时，这时要如实说。
LOOK_TIMEOUT_MS = 8000
# 玩家说话时顺手取快照的超时：给得很短，玩家那句话不能被一次查询拖住（游戏暂停时
# 模组答不上来，这一轮就不带，照常把话送进对话）。
CARRY_TIMEOUT_MS = 1500

# 玩家离她这么近才算「在身边」。只判距离，不问「她看不看得见他」——视线那条会让模型
# 去编「我看见你了」，而真正要紧的只是主人有没有走远。
OWNER_NEAR = 30.0

# 「你此刻：附近有…」里最多念几只活物。模组两档都只给最近 3 个，这里再兜一层。
CARRY_CREATURE_LIMIT = 3

LOOK_AROUND_TOOL = "maid_look_around"
SEE_SCREEN_TOOL = "maid_see_screen"
SET_TASK_TOOL = "maid_set_task"
ATTACK_TOOL = "maid_attack"
MINE_TOOL = "maid_mine"
GIVE_TOOL = "maid_give_item"

# 抓屏时按这个关键词找游戏窗口（标题或进程名命中即可），找不到就抓整个屏幕。
DEFAULT_SCREEN_WINDOW = "Minecraft"

# 图交给宿主要附的那句话：图从哪来、看到的是什么，模型才知道该怎么用。
SCREEN_HINT = (
    "（这是玩家屏幕上此刻的样子，也就是你眼前看到的画面。"
    "图里如果有聊天窗口，那是你自己在的那扇窗，不用管它。）"
)

# 带目标的动作与各自的默认目标。执行器只校验 JSON Schema 的一个子集、不认 enum，
# 所以合法值在这里兜底（模组那头还会再校验一次）。
TARGET_DEFAULTS = {"go_to": "player", "lie_down": "bed"}

# 受理了但还没做完时给模型的话：必须让它知道自己还没到位，否则它会当场宣布已经办到。
# 只陈述此刻的状态，不承诺「之后会通知」——模型会照着这句对玩家说出口。
PENDING_TEXT = {
    "go_to": "她正在走过去，还没到玩家身边。",
    "lie_down": "她正在往床边走，还没躺下。",
}

# 受理即完成的动作给模型的话。
DONE_TEXT = {
    "sit": "已经坐下了。",
    "stand": "已经站起来了。",
}


def run(ctx):
    """工具入口。认不出的工具如实拒绝，不猜。"""
    tool = (ctx.get("tool_name") or "").strip()
    handler = TOOLS.get(tool)
    if handler is None:
        return {"ok": False, "error": f"这个工具不存在：{tool}"}
    return handler(ctx)


def action_tool(action):
    """把一个动作包成工具入口：工具名带插件前缀，动作名不带。"""

    def handler(ctx):
        return _do_action(ctx, action)

    return handler


def _do_action(ctx, action):
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    data = {"maid_id": maid, "action": action}
    target = _action_target(action, ctx.get("args") or {})
    if target:
        data["target"] = target

    response, error = ask({"type": "maid_action", "data": data}, ACTION_TIMEOUT_MS)
    if error:
        return {"ok": False, "error": error}
    frame = response.get("data") or {}
    if not frame.get("accepted"):
        return {"ok": False, "error": frame.get("reason") or "游戏拒绝了这次动作"}
    if frame.get("async"):
        return {"ok": True, "message": PENDING_TEXT.get(action, "已经开始，还没做完。")}
    return {"ok": True, "message": DONE_TEXT.get(action, "已经做完了。")}


def _action_target(action, args):
    """这次动作要带的目标；不带目标的动作返回空串。

    认不出的取值一律退回默认：执行器的 schema 校验不认 enum，模型可能塞别的字符串。
    """
    default = TARGET_DEFAULTS.get(action)
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


def _set_task(ctx):
    """换她的工作模式。切的就是她自己界面里那个任务，我们这边不留副本。"""
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    raw = (ctx.get("args") or {}).get("task")
    raw = raw.strip() if isinstance(raw, str) else ""
    task = task_id(raw)
    if not task:
        return {"ok": False, "error": f"没有这个工作模式：{raw or '（空）'}。能用的有：{task_choices()}。"}

    response, error = ask({"type": SET_TASK_TOOL, "data": {"maid_id": maid, "task": task}}, ACTION_TIMEOUT_MS)
    if error:
        return {"ok": False, "error": error}
    frame = response.get("data") or {}
    if response.get("type") == "error":
        # 模组没就绪，或者这个模组版本还不认这条帧
        return {"ok": False, "error": (frame.get("message") or "").strip() or "游戏那边没能换工作模式"}
    if not frame.get("accepted"):
        return {"ok": False, "error": frame.get("reason") or "游戏拒绝了这次切换"}
    return {"ok": True, "message": _task_text(frame)}


def _task_text(frame):
    """换工作的回执 → 给模型的一句话：成没成、缺不缺东西、现在会不会真去干。

    后两件事模型自己看不见，而它们决定玩家会不会白等：她的日程表此刻可能不在工作时间；
    跟随状态下有活时，玩家走远了不会把她拽回来，活会接着干，叫一声就能回来。
    """
    name = task_name(frame.get("task"), frame.get("summary"))
    parts = [f"已经切换到「{name}」了。" if frame.get("changed") else f"本来就在「{name}」。"]
    switch = (frame.get("switch_result") or "").strip()
    if switch == "MISSING_REQUIRED_ITEM":
        parts.append("不过手上和背包里都没有要用的东西（锄头、剪刀、钓竿这类），得先给你。")
    elif switch == "PARTIAL_OK":
        parts.append("不过工具不全，只有一部分活能干。")
    if frame.get("work_time") is False:
        parts.append("不过现在是你的休息时间，到了工作时间才会去干。")
    elif frame.get("home_mode") is False:
        parts.append("不过你还在跟随模式：干活的时候玩家走远了不会把你拽回来，活会接着干，叫一声就回来。")
    return "".join(parts)


def _attack(ctx):
    """让她去打一个活物。点谁由玩家那句话决定，插件只把说法翻成 id 或类别。"""
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    raw = (ctx.get("args") or {}).get("target")
    raw = raw.strip() if isinstance(raw, str) else ""
    target = attack_target(raw)
    if not target:
        return {
            "ok": False,
            "error": f"没听懂要打谁：{raw or '（空）'}。可以说活物的名字（僵尸、羊），"
            "或者 hostile（最近的敌对生物）、animal（最近的动物）、owner（主人，也就是玩家自己）。",
        }
    style = attack_style((ctx.get("args") or {}).get("style"))

    response, error = ask(
        {"type": ATTACK_TOOL, "data": {"maid_id": maid, "target": target, "style": style}}, ACTION_TIMEOUT_MS
    )
    if error:
        return {"ok": False, "error": error}
    frame = response.get("data") or {}
    if response.get("type") == "error":
        return {"ok": False, "error": (frame.get("message") or "").strip() or "游戏那边没能下达攻击"}
    if not frame.get("accepted"):
        return {"ok": False, "error": frame.get("reason") or "游戏拒绝了这次攻击"}
    return {"ok": True, "message": _attack_text(frame)}


def _attack_text(frame):
    """攻击回执 → 一句话。目标只说名字，不给格数。

    说清这个工具**只负责攻击**：它不保证目标已经倒下。模型很容易把「盯上了」当成
    「已经解决」，而实际上她可能才砍到第一下。

    轻轻一下是另一回事：只出一手、伤害固定，说清「打完就收手」就够，不必再提倒下。
    """
    target = frame.get("target") or {}
    if target.get("owner"):
        who = "玩家"
    else:
        fallback = (target.get("name") or "").strip() or "那个活物"
        who = name_of(MOB_NAMES, target.get("type"), fallback)
    if frame.get("style") == "light":
        return f"会过去轻轻打{who}一下，打完就收手，不会追着打。"
    parts = [f"已开始攻击{who}。"]
    if not frame.get("weapon", True):
        parts.append("她手上没有武器，只能空手打，伤害很低。")
    parts.append(f"这个工具只下达攻击指令，{who}还没有倒下；她会一直打到目标倒下或追不上为止。")
    return "".join(parts)


def _mine(ctx):
    """让她去挖方块：玩家看着的那块，或附近的某一种方块。"""
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    args = ctx.get("args") or {}
    raw = args.get("target")
    raw = raw.strip() if isinstance(raw, str) else ""
    target = mine_target(raw)
    if not target:
        return {
            "ok": False,
            "error": f"没听懂要挖哪种方块：{raw or '（空）'}。可以说 {mine_choices()}。",
        }
    try:
        count = int(args.get("count"))
    except (TypeError, ValueError):
        count = 1
    count = max(1, min(64, count))

    response, error = ask(
        {"type": MINE_TOOL, "data": {"maid_id": maid, "target": target, "count": count}},
        ACTION_TIMEOUT_MS,
    )
    if error:
        return {"ok": False, "error": error}
    frame = response.get("data") or {}
    if response.get("type") == "error":
        return {"ok": False, "error": (frame.get("message") or "").strip() or "游戏那边没能开始挖"}
    if not frame.get("accepted"):
        return {"ok": False, "error": frame.get("reason") or "游戏拒绝了这次挖掘"}
    return {"ok": True, "message": _mine_text(frame)}


def _mine_text(frame):
    """挖掘回执 → 一句话。这个工具只负责开始挖，不代表方块已经掉下来。

    挖穿、挖够、走不到都由模组之后推的 `maid_mine_done` 状态说明；在这之前，
    模型只能说她「正过去挖」。
    """
    block = frame.get("block") or ""
    fallback = (frame.get("name") or "").strip() or "方块"
    name = name_of(BLOCK_NAMES, block, fallback)
    if frame.get("looked"):
        parts = [f"正过去挖你看着的那块{name}。"]
    else:
        parts = [f"正过去挖{frame.get('count') or 1}块{name}。"]
    if not frame.get("has_tool", True):
        parts.append("她手上没有趁手的工具，只能徒手挖，会慢一些。")
    parts.append(f"这个工具只表示开始挖了，{name}还在原地。")
    return "".join(parts)


def _give_item(ctx):
    """把她背包里的东西交给玩家：同步完成，成败当场给答复。"""
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    args = ctx.get("args") or {}
    raw = args.get("item")
    raw = raw.strip() if isinstance(raw, str) else ""
    target = item_target(raw)
    if not target:
        if not raw:
            return {"ok": False, "error": f"没说要给你哪件东西。可以说 {item_choices()}。"}
        # 认不出的名字照样发给模组：失败回执会带背包清单，模型能按 id 再来一次
        target = raw
    try:
        count = int(args.get("count"))
    except (TypeError, ValueError):
        count = 1
    count = max(1, min(64, count))

    response, error = ask(
        {"type": GIVE_TOOL, "data": {"maid_id": maid, "item": target, "count": count}},
        ACTION_TIMEOUT_MS,
    )
    if error:
        return {"ok": False, "error": error}
    frame = response.get("data") or {}
    if response.get("type") == "error":
        return {"ok": False, "error": (frame.get("message") or "").strip() or "游戏那边没能把东西交给你"}
    if not frame.get("accepted"):
        return {"ok": False, "error": _give_error(frame)}
    return {"ok": True, "message": _give_text(frame)}


def _give_text(frame):
    """交物品的回执 → 一句话。只说她真的给了多少，不替玩家收下。"""
    name = item_name(frame.get("item"))
    given = as_int(frame.get("count"), 1)
    wanted = as_int(frame.get("requested"), given)
    if given < wanted:
        return f"她身上只有{given}个{name}，都交给你了。"
    return f"把{given}个{name}交给了你。"


def _give_error(frame):
    """失败回执 → 原因；模组带了背包清单时一并列出，模型下一句就能按 id 改口。"""
    reason = (frame.get("reason") or "").strip() or "游戏拒绝了这次交接"
    available = [one for one in (frame.get("available") or []) if isinstance(one, dict) and one.get("item")]
    if not available:
        return reason
    listed = "、".join(
        f"{item_name(one.get('item'))}（{one.get('item')}）×{as_int(one.get('count'), 1)}"
        for one in available
    )
    return f"{reason}。她背包里有：{listed}。要哪件就按 id 再说。"


def _look_around(ctx):
    """看一眼她周围：取一份快照翻成几句事实给模型。

    只回去给模型看，不写台词表、也不请谁开口——玩家正在跟她说话，这一轮本来就要回话，
    再写一条旁白就是重复。
    """
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    data, error = snapshot(maid, LOOK_TIMEOUT_MS, "query")
    if error:
        return {"ok": False, "error": error}
    return {"ok": True, "message": _look_around_text(data)}


def _see_screen(ctx):
    """看一眼玩家此刻屏幕上的画面，把图交给模型。

    图不经过这段文字：宿主会把工具结果里的 image 摘出来，单起一条消息附给模型，
    只在这一轮生效、不进长期记忆。抓不到就如实说，别让模型照着想象编。
    """
    window = (config(ctx).get("screen_window") or "").strip()
    shot = capture_screen(window=window or DEFAULT_SCREEN_WINDOW)
    if not shot.get("ok"):
        return {"ok": False, "error": shot.get("error") or "抓不到画面"}
    image = (shot.get("image") or "").strip()
    if not image:
        return {"ok": False, "error": "抓回来的画面是空的"}
    return {
        "ok": True,
        "message": "你看了一眼玩家此刻屏幕上的画面，图已经附上了。",
        "image": {"data_url": image, "hint": SCREEN_HINT},
    }


def snapshot(maid_id, timeout_ms, profile="auto"):
    """现取一份她身边的快照。返回 (数据, 错误说明)；出错时数据是 None。

    工具「看一眼周围」与「玩家开口时顺手带一份」共用这条路：帧、超时口径、老模组
    认不出这个帧时的说法都只有一处。profile 决定模组用哪一档：auto 是开口时顺手带的
    近处快照（看得见按工作范围、看不见只听很近的声音），query 是玩家明确问周围有什么
    （看得见放宽到 32 格，并附方块与背包清单）。
    """
    data = {"maid_id": maid_id, "profile": profile}
    response, error = ask({"type": LOOK_AROUND_TOOL, "data": data}, timeout_ms)
    if error:
        return None, error
    if response.get("type") == "error":
        # 老模组认不出这个帧，回的是统一错误帧（里面只有 message，没有业务字段）
        message = ((response.get("data") or {}).get("message") or "").strip()
        return None, f"游戏那边没能回答：{message or '这个模组版本还不支持查看周围'}"
    return response.get("data") or {}, ""


def carry_text(data):
    """一份快照 → 一句「你此刻的处境」，附在玩家的发言后面。

    只挑此刻要紧的说：脚下与头顶、玩家在不在身边、附近有什么活物。正前方那一眼留给
    工具那句——玩家问起来才需要那么细，每句话都带上就成了噪音。
    帧里没有 owner 字段说明这压根不是快照（多半是别的东西回了信），那就什么都不带。
    """
    if "owner" not in data:
        return ""
    parts = []
    foot = _footing_phrase(data)
    if foot:
        parts.append(foot)
    parts.append(_owner_phrase(data.get("owner") or {}))
    creatures = creature_phrase(data)
    if creatures:
        parts.append(creatures)
    return "（你此刻：" + "；".join(parts) + "）"


def creature_phrase(data):
    """附近有哪些活物，一句话的中间那半截（「附近有近处的僵尸」）。

    出事那一刻（她挨打、血量告急）跟着事件旁白一起给她——怪物另记一条有 2 分钟防抖
    挡着，但「她刚挨了一下，旁边站着谁」是当下最要紧的处境，不该等到玩家下次说话。

    看得见的按距离档报；看不见的不报「被挡着」，只汇总成「听到僵尸和蜘蛛的声音」——
    隔墙知道具体是哪一只、离多远，读起来像透视，也不像她真有的感官。

    只看最近几只：按需查询现在能列十几只，全念出来会把玩家那句话挤成一份清单。
    """
    nearby = (data.get("nearby") or [])[:CARRY_CREATURE_LIMIT]
    if not nearby:
        return ""
    seen = [one for one in nearby if one.get("visible")]
    sounds = heard_sounds([one for one in nearby if not one.get("visible")])
    parts = []
    if seen:
        parts.append(f"附近有{'、'.join(sighting(one) for one in seen)}")
    if sounds:
        parts.append(f"{'还能' if seen else '附近能'}听到{sounds}")
    return "；".join(parts)


def creature_text(data):
    """同上，但成句：没有活物就什么都说不出来（空串）。"""
    phrase = creature_phrase(data)
    return f"{phrase}。" if phrase else ""


def _look_around_text(data):
    """她身边这一圈，说成几句事实：站位 → 正前方 → 能挖的方块 → 背包 → 玩家 → 活物。

    数字只出现在物品数量上：格数、血量、光照写进上下文，模型就会照本宣科地念出来。
    """
    dimension = name_of(DIMENSION_NAMES, data.get("dimension"), "某个世界")
    foot = name_of(BLOCK_NAMES, (data.get("standing_on") or {}).get("id"), "说不清的东西")
    roof = "头顶有遮挡，看不到天" if data.get("sheltered") else "头顶是露天的"
    lines = [f"你正站在{dimension}，脚下是{foot}，{roof}。"]

    look = data.get("looking_at") or {}
    look_kind = (look.get("kind") or "").strip()
    if look_kind == "block":
        block = name_of(BLOCK_NAMES, look.get("id"), "一个方块")
        lines.append(f"正前方{nearness_place(look.get('distance'))}是{block}。")
    elif look_kind == "entity":
        entity = name_of(MOB_NAMES, look.get("id"), "一个活物")
        lines.append(f"正前方{nearness_place(look.get('distance'))}是{entity}。")
    else:
        lines.append("正前方没有挡着的东西，视线是通的。")

    # 老模组不返回 blocks：这时不能替它说「没有」，跳过这一句
    blocks = data.get("blocks")
    listed = [one for one in (blocks or []) if isinstance(one, str) and one]
    if listed:
        lines.append(_blocks_text(listed))
    elif blocks is not None:
        lines.append("附近没看到能挖的方块。")

    # 老模组同样没有 backpack：不替它说「背包是空的」
    backpack = data.get("backpack")
    if backpack is not None:
        lines.append(_backpack_text(backpack))

    lines.append(_owner_phrase(data.get("owner") or {}) + "。")

    creatures = creature_text(data)
    lines.append(creatures if creatures else "附近没有别的活物。")
    if data.get("truncated"):
        lines.append("更远的没列出来。")
    return "".join(lines)


def _blocks_text(blocks):
    """附近能挖的方块 → 一句带 id 的清单，模型可以直接拿 id 去调 maid_mine。"""
    listed = []
    for raw in blocks:
        name = BLOCK_NAMES.get(raw)
        listed.append(f"{name}（{raw}）" if name else raw)
    return "附近能挖的方块有" + "、".join(listed) + "。"


def _backpack_text(items):
    """她背包里有什么 → 一句带 id 的清单，模型可以直接拿 id 去调 maid_give_item。"""
    listed = []
    for one in items or []:
        if not isinstance(one, dict):
            continue
        raw = (one.get("item") or "").strip()
        if not raw:
            continue
        count = as_int(one.get("count"), 1)
        piece = f"{item_name(raw)}（{raw}）"
        listed.append(f"{piece}×{count}" if count > 1 else piece)
    return "你的背包里有" + "、".join(listed) + "。" if listed else "你的背包里没有别的东西。"


def _footing_phrase(data):
    """脚下踩着哪。「近/中/远」的档位在这句里没意义，所以单独拎出来。"""
    foot = name_of(BLOCK_NAMES, (data.get("standing_on") or {}).get("id"), "")
    if not foot:
        return ""
    # 下界与末地没有天空，「头顶有没有遮挡」在那里无从谈起
    roof = ""
    if data.get("has_sky"):
        roof = "，头顶有遮挡" if data.get("sheltered") else "，头顶是露天的"
    return f"脚下是{foot}{roof}"


def _nearness(distance):
    """「近处」/「不远处」/「远处」，塞进「正前方 X 是…」这种句子用。"""
    return {"近": "近处", "中": "不远处", "远": "远处"}.get(nearness(distance), "不远处")


def _owner_phrase(owner):
    """主人在不在她身边（`OWNER_NEAR` 之内），不带句号。

    只说在不在，不给格数，也不问视线：两条路（工具与开口时携带）共用这一句，
    说法只有一处。
    """
    if not owner.get("same_dimension"):
        return "玩家不在你所在的这个世界里"
    try:
        distance = float(owner.get("distance"))
    except (TypeError, ValueError):
        return "你身边没有玩家"
    if 0 <= distance <= OWNER_NEAR:
        return "玩家就在你身边"
    return "玩家不在你身边"


TOOLS = {
    "maid_go_to": action_tool("go_to"),
    "maid_sit_down": action_tool("sit"),
    "maid_stand_up": action_tool("stand"),
    "maid_lie_down": action_tool("lie_down"),
    LOOK_AROUND_TOOL: _look_around,
    SEE_SCREEN_TOOL: _see_screen,
    SET_TASK_TOOL: _set_task,
    ATTACK_TOOL: _attack,
    MINE_TOOL: _mine,
    GIVE_TOOL: _give_item,
}
