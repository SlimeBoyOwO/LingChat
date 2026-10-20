"""查询工具：看一眼她周围、看一眼玩家屏幕。"""

from plugin_host import capture_screen

from bridge import config, ready
from naming import names
from tools.common import LOOK_TIMEOUT_MS
from tools.context import LOOK_AROUND_TOOL, creature_text, owner_phrase, snapshot
from wording import (
    BLOCK_NAMES,
    DIMENSION_NAMES,
    MOB_NAMES,
    as_int,
    item_name,
    light_phrase,
    name_of,
    nearness_place,
)

SEE_SCREEN_TOOL = "maid_see_screen"

# 抓屏时按这个关键词找游戏窗口（标题或进程名命中即可），找不到就抓整个屏幕。
DEFAULT_SCREEN_WINDOW = "Minecraft"

# 图交给宿主要附的那句话：图从哪来、看到的是什么，模型才知道该怎么用。
SCREEN_HINT = (
    "（这是{player}屏幕上此刻的样子，也就是{maid}眼前看到的画面。"
    "图里如果有聊天窗口，那是{maid}自己在的那扇窗，不用管它。）"
)


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
    maid, player = names()
    window = (config(ctx).get("screen_window") or "").strip()
    shot = capture_screen(window=window or DEFAULT_SCREEN_WINDOW)
    if not shot.get("ok"):
        return {"ok": False, "error": shot.get("error") or "抓不到画面"}
    image = (shot.get("image") or "").strip()
    if not image:
        return {"ok": False, "error": "抓回来的画面是空的"}
    return {
        "ok": True,
        "message": f"{maid}看了一眼{player}此刻屏幕上的画面，图已经附上了。",
        "image": {
            "data_url": image,
            "hint": SCREEN_HINT.format(maid=maid, player=player),
        },
    }


def _look_around_text(data):
    """她身边这一圈，说成几句事实：站位与光线 → 正前方 → 能挖的方块 → 背包 → 玩家 → 活物。

    距离只给近 / 中 / 远，格数与血量不写：数字一进上下文，模型就会照本宣科地念出来。
    光照也只给档位（明亮 / 昏暗 / 漆黑），不给 0-15 的原始数值。
    """
    maid, player = names()
    dimension = name_of(DIMENSION_NAMES, data.get("dimension"), "某个世界")
    foot = name_of(BLOCK_NAMES, (data.get("standing_on") or {}).get("id"), "说不清的东西")
    # 下界与末地没有天空，不谈头顶
    roof = "" if data.get("has_sky") is False else (
        "头顶有遮挡，看不到天" if data.get("sheltered") else "头顶是露天的"
    )
    light = light_phrase(data.get("light"))
    place = "，".join(one for one in (f"脚下是{foot}", roof, light) if one)
    lines = [f"{maid}正站在{dimension}，{place}。"]

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
        lines.append(_backpack_text(backpack, maid))

    lines.append(owner_phrase(data.get("owner") or {}, maid, player) + "。")

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


def _backpack_text(items, maid):
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
    return f"{maid}的背包里有" + "、".join(listed) + "。" if listed else f"{maid}的背包里没有别的东西。"


TOOLS = {
    LOOK_AROUND_TOOL: _look_around,
    SEE_SCREEN_TOOL: _see_screen,
}
