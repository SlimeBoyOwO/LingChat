"""查询工具：看一眼她周围、看一眼玩家屏幕。"""

from plugin_host import capture_screen

from bridge import ask, config, ready
from naming import names
from tools.common import LOOK_TIMEOUT_MS, STATE_TIMEOUT_MS
from tools.context import LOOK_AROUND_TOOL, creature_text, owner_phrase, remembered_text, snapshot
from wording import (
    BLOCK_NAMES,
    DIMENSION_NAMES,
    DIRECTION_NAMES,
    MOB_NAMES,
    OVERHEAD_PLACES,
    STRUCTURE_NAMES,
    VEHICLE_NAMES,
    as_int,
    blocker_phrase,
    item_name,
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

    args = ctx.get("args") or {}
    raw_detail = args.get("detail")
    detail = raw_detail.strip().lower() if isinstance(raw_detail, str) else "brief"
    if detail not in ("brief", "map"):
        detail = "brief"
    data, error = snapshot(maid, LOOK_TIMEOUT_MS, "query", detail)
    if error:
        return {"ok": False, "error": error}
    return {"ok": True, "message": _look_around_text(data)}


def _see_screen(ctx):
    """看一眼玩家此刻屏幕上的画面，把图交给模型。

    图不经过这段文字：宿主会把工具结果里的 image 摘出来，单起一条消息附给模型，
    只在这一轮生效、不进长期记忆。抓不到就如实说，别让模型照着想象编。
    """
    maid, player = names()
    # 昏迷或死亡时她看不见玩家的屏幕；问一句模组，问不到（游戏暂停）就照常抓
    state, error = ask({"type": "maid_state", "data": {}}, STATE_TIMEOUT_MS)
    if not error and state and state.get("type") == "maid_state_result":
        data = state.get("data") or {}
        if data.get("known") and (data.get("dead") or data.get("unconscious")):
            return {"ok": False, "error": f"{maid}现在不省人事，看不到{player}屏幕上的画面"}
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
    """她身边这一圈，说成几句事实：姿势与站位 → 头顶 → 正前方 → 最近一块的关系
    → 成团清单 → 方块清单 → 问路小图 → 背包 → 玩家 → 活物。

    活物与正前方仍只说近 / 中 / 远；方块的方位与步数是模组算好的关系，只在被问到
    怎么走时才有用，模型不必自己数。光线不在这里说：它只在跨档时由 light_change
    旁白交代一次，工具再念一遍，同一轮里就会重复两条一样的话。坐在船、矿车里时说载具，
    不说脚下踩着什么——那时候脚底下本来就没有「地」。
    """
    maid, player = names()
    dimension = name_of(DIMENSION_NAMES, data.get("dimension"), "某个世界")
    foot = name_of(BLOCK_NAMES, (data.get("standing_on") or {}).get("id"), "说不清的东西")
    vehicle = (data.get("vehicle") or "").strip()
    roof = _overhead_text(data)
    if vehicle:
        seat = f"{maid}正坐在{dimension}的{name_of(VEHICLE_NAMES, vehicle, '载具')}里"
        lines = [f"{seat}，{roof}。" if roof else f"{seat}。"]
    else:
        place = "，".join(one for one in (f"脚下踩着{foot}", roof) if one)
        lines = [f"{_posture_subject(data, maid, dimension)}，{place}。"]
    structure = (data.get("structure") or "").strip()
    if structure:
        lines.append(f"这里是一处{name_of(STRUCTURE_NAMES, structure, '结构')}。")

    look = data.get("looking_at") or {}
    look_kind = (look.get("kind") or "").strip()
    if look_kind == "block":
        block = name_of(BLOCK_NAMES, look.get("id"), "一个方块")
        lines.append(f"正前方{nearness_place(look.get('distance'))}是{block}。")
    elif look_kind == "entity":
        entity = name_of(MOB_NAMES, look.get("id"), "一个活物")
        lines.append(f"正前方{nearness_place(look.get('distance'))}是{entity}。")
    else:
        lines.append("正前方空着，什么也没挡着。")

    # 新模组附带的关系与相连团：先给最近那一块的方位与步数，再补一句另一团的大小。
    # 老模组没有这两项，下面照旧只念方块清单。
    relations = data.get("relations")
    clusters = data.get("clusters")
    clusters = clusters if isinstance(clusters, list) else []
    if isinstance(relations, list) and relations:
        relation_text = _relation_text(relations, clusters)
        if relation_text:
            lines.append(relation_text)
        cluster_text = _cluster_text(clusters, relations)
        if cluster_text:
            lines.append(cluster_text)

    # 认得 block_sight 时把「此刻看见」与「刚才看见、现在不在视野里」分开说；
    # 老模组只有 blocks，照旧全当成眼前这一片。
    sights = data.get("block_sight")
    blocks = data.get("blocks")
    if isinstance(sights, list) and sights:
        now = []
        before = []
        for one in sights:
            if not isinstance(one, dict):
                continue
            raw = (one.get("id") or "").strip()
            if not raw:
                continue
            (before if one.get("seen") == "before" else now).append(raw)
        if now:
            lines.append(_blocks_text(now))
        if before:
            lines.append(_blocks_text(before, "刚才在附近见过"))
        if not now and not before:
            lines.append("附近没看到能挖的方块。")
    else:
        # 老模组不返回 blocks：这时不能替它说「没有」，跳过这一句
        listed = [one for one in (blocks or []) if isinstance(one, str) and one]
        if listed:
            lines.append(_blocks_text(listed))
        elif blocks is not None:
            lines.append("附近没看到能挖的方块。")

    # 问路时才要的小图：模组按她脚下那一层画好，这里只起个头
    map_info = data.get("map")
    if isinstance(map_info, dict):
        map_text = (map_info.get("text") or "").strip("\n")
        if map_text:
            lines.append(
                f"{maid}脚下的这一片（@ 是她，. 可走，# 挡住，~ 水，! 岩浆，? 没加载）：\n{map_text}"
            )

    # 老模组同样没有 backpack：不替它说「背包是空的」
    backpack = data.get("backpack")
    if backpack is not None:
        lines.append(_backpack_text(backpack, maid, data.get("headwear")))

    lines.append(owner_phrase(data.get("owner") or {}, maid, player) + "。")

    creatures = creature_text(data)
    lines.append(creatures if creatures else "附近没有别的活物。")
    remembered = remembered_text(data.get("remembered") or [])
    if remembered:
        lines.append(remembered)
    if data.get("truncated"):
        lines.append("更远的没再细看。")
    return "".join(lines)


def _overhead_text(data):
    """头顶这一句：露天 / 半遮（树荫、屋檐、洞口）/ 屋内（房间、洞穴）。

    三档与头顶那块方块由模组判定，这里只挑说法；老模组没有 `overhead` 时退回布尔。
    下界与末地没有天空，整句不说。
    """
    if data.get("has_sky") is False:
        return ""
    kind = (data.get("overhead") or "").strip()
    if not kind:
        return "头顶有遮挡，看不到天" if data.get("sheltered") else "头顶是露天的"
    if kind == "open":
        return "头顶是露天的"
    cover = name_of(BLOCK_NAMES, data.get("overhead_cover"), "")
    place = OVERHEAD_PLACES.get((data.get("overhead_band") or "").strip(), "头顶")
    if kind == "partial":
        if cover:
            return f"{place}有{cover}遮着一部分，还能看见天"
        return "头顶遮着一部分，还能看见天"
    if cover:
        return f"{place}被{cover}封着，看不到天"
    return "头顶封着，看不到天"


def _posture_subject(data, maid, dimension):
    """她此刻是什么姿势：坐着、睡着、躺在地上时不能再说「正站在」。"""
    posture = (data.get("posture") or "").strip()
    if posture == "lying":
        return f"{maid}正躺在地上"
    if posture == "sitting":
        return f"{maid}正坐着"
    if posture == "sleeping":
        return f"{maid}正睡着"
    return f"{maid}正站在{dimension}"


def _pick_relation(relations):
    """最近那一块：优先此刻看见的，没有就退回刚才见过的第一条。"""
    for one in relations:
        if isinstance(one, dict) and one.get("seen") == "now" and (one.get("id") or "").strip():
            return one
    for one in relations:
        if isinstance(one, dict) and (one.get("id") or "").strip():
            return one
    return None


def _distance_steps(one):
    try:
        return max(0, int(round(float(one.get("distance") or 0))))
    except (TypeError, ValueError):
        return 0


def _where_text(one):
    """关系里的「正前方三步」；没有方位就只说步数，站在同一格说「就在跟前」。"""
    facing = DIRECTION_NAMES.get((one.get("facing") or "").strip(), "")
    steps = _distance_steps(one)
    if steps <= 0:
        return "就在跟前"
    return f"{facing}{steps}步" if facing else f"{steps}步外"


def _layer_text(dy):
    if dy == 0:
        return "同一层"
    return f"高{dy}层" if dy > 0 else f"低{-dy}层"


def _blocker_text(one):
    phrase = blocker_phrase(one)
    return "中间没有挡着" if not phrase else "中间" + phrase


def _at_of(one):
    """关系里的那一格 → maid_mine 能用的相对地址；老模组没有 at 时按 dx/dy/dz 拼。

    地址相对她脚下那一格，模组据此核对「指的是不是这块」。
    """
    at = (one.get("at") or "").strip()
    if at:
        return at
    dx, dy, dz = one.get("dx"), one.get("dy"), one.get("dz")
    if all(isinstance(value, (int, float)) for value in (dx, dy, dz)):
        return f"@{int(dx)},{int(dy)},{int(dz)}"
    return ""


def _cluster_of(clusters, block_id):
    for one in clusters:
        if isinstance(one, dict) and (one.get("id") or "").strip() == block_id:
            return one
    return None


def _cluster_size(cluster):
    cells = as_int(cluster.get("cells"), 0)
    text = f"一共{cells}块"
    if cluster.get("truncated"):
        text += "以上"
    return text


def _relation_text(relations, clusters):
    """最近那一块的关系句：方位、层高、挡路与步数；成团时把团的大小并进来。

    只挑一块说，不把十二种方块都写成句子——清单仍然是清单，关系只给最近的那块。
    """
    picked = _pick_relation(relations)
    if not picked:
        return ""
    raw = (picked.get("id") or "").strip()
    name = name_of(BLOCK_NAMES, raw, "方块")
    where = _where_text(picked)
    if picked.get("seen") == "before":
        text = f"刚才在{where}见过{name}（{raw}）"
        blocker = blocker_phrase(picked)
        if blocker:
            text += f"，现在{blocker}"
        return text + "。" + _at_sentence(_at_of(picked))
    text = f"{where}、{_layer_text(as_int(picked.get('dy'), 0))}是{name}（{raw}）"
    cluster = _cluster_of(clusters, raw)
    # 数到上限的团多半是石头、泥土这种大片地形，不是「找到一处矿」；不报大小
    if cluster and 1 < as_int(cluster.get("cells"), 0) and not cluster.get("truncated"):
        text += "，这一团" + _cluster_size(cluster)
    text += "，" + _blocker_text(picked)
    walk = as_int(picked.get("walk_steps"), -1)
    if walk == 0:
        text += "，就在跟前"
    elif walk > 0:
        text += f"，再走{walk}步就到"
    return text + "。" + _at_sentence(_at_of(picked))


def _at_sentence(at):
    """把地址交给模型：maid_mine 的 at 填这个就指到那一块。"""
    return f"这一块是 at={at}。" if at else ""


def _cluster_text(clusters, relations):
    """再补一团另一种方块的大小；与关系句重复的那一团不再说。"""
    picked = _pick_relation(relations)
    picked_id = (picked.get("id") or "").strip() if picked else ""
    for one in clusters:
        if not isinstance(one, dict):
            continue
        raw = (one.get("id") or "").strip()
        if not raw or raw == picked_id or as_int(one.get("cells"), 0) <= 1:
            continue
        # 同上：数到上限的大片地形不单独成句
        if one.get("truncated"):
            continue
        name = name_of(BLOCK_NAMES, raw, "方块")
        text = f"{_where_text(one)}的{name}团{_cluster_size(one)}"
        blocker = blocker_phrase(one)
        if blocker:
            text += f"，最近的一块{blocker}"
        text += "。"
        at = _at_of(one)
        return text + (f"最近那块是 at={at}。" if at else "")
    return ""


def _blocks_text(blocks, lead="眼前这一片能挖的有"):
    """能挖的方块 → 一句带 id 的清单，模型可以直接拿 id 去调 maid_mine。

    「刚才在附近见过」那一路同样是带出处的：她确实看见过，只是现在不在视野里，
    不是替模组做全知扫描。
    """
    listed = []
    for raw in blocks:
        name = BLOCK_NAMES.get(raw)
        listed.append(f"{name}（{raw}）" if name else raw)
    return lead + "、".join(listed) + "。"


def _backpack_text(items, maid, headwear=None):
    """她背包里有什么 → 一句带 id 的清单，模型可以直接拿 id 去调 maid_give_item。

    最后一格是头饰栏，那一格里的东西顶在头上，不算背包存货，所以单独说。
    """
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
    text = f"{maid}的背包里有" + "、".join(listed) + "。" if listed else f"{maid}的背包里没有别的东西。"
    head = (headwear or {}).get("item") if isinstance(headwear, dict) else ""
    if (head or "").strip():
        text += f"头饰栏（背包最后一格，顶在头上）里是{item_name(head)}（{head.strip()}）。"
    return text


TOOLS = {
    LOOK_AROUND_TOOL: _look_around,
    SEE_SCREEN_TOOL: _see_screen,
}
