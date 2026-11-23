"""建筑工具：固定小木屋的现场施工，以及把复杂建筑交给导演生成蓝图。

这一期先用一张写死的小木屋图纸跑通「插件发图纸、模组自己盖」：`maid_build_house`
立刻返回，模组按 tick 一块块放，缺料、够不着或盖完才通过事件旁白再叫 LLM。
导演那条链（`maid_plan_building` / `maid_building_status`）保持不变，蓝图以后接同一条
施工帧；提示词与提交工具 schema 都由本插件组装，宿主只跑通用任务循环。
"""

import json

from plugin_host import director_task_start, director_task_status

from bridge import ask, ready
from naming import names, narrate
from tools.common import ACTION_TIMEOUT_MS, LOOK_TIMEOUT_MS, STATE_TIMEOUT_MS
from tools.context import snapshot
from wording import as_int

PLAN_TOOL = "maid_plan_building"
STATUS_TOOL = "maid_building_status"
BUILD_TOOL = "maid_build_house"
CLIMB_TOOL = "maid_build_climb_test"

# 施工帧与固定图纸。这一期先把蓝图写死成一座 11×11 的中型木屋，用来验证模组侧
# 能不能不经过 LLM 一块块盖起来；导演生成的蓝图以后走同一条施工帧。
BUILD_FRAME = "maid_build"
HOUSE_NAME = "小木屋"
# 垫脚测试柱：2×2、8 格高。下面几层从地面够得着，最上面两层只能先垫一块走上去，
# 用来验证施工器「垫一格再走上去」这一条在真机上成不成立。
CLIMB_NAME = "垫脚测试柱"
CLIMB_BLOCK = "minecraft:cobblestone"
CLIMB_SIDE = 2
CLIMB_HEIGHT = 8
# 图纸按正方形方格旋转，HOUSE_SIDE 既是方格边长也是旋转参照，换成非正方形就对不上中心。
HOUSE_SIDE = 11
# 施工起点放在她正前方几格，房子以锚点为中心，免得一开始就把她圈在地基里；
# 11×11 的房子连同外挑一格的屋檐前后各占 6 格，留 9 格门廊才不会压在她脚下。
ANCHOR_DISTANCE = 9
# 四个朝向（原版 yaw 每 90 度一档）的前进方向：南、西、北、东。
FORWARD = ((0, 1), (-1, 0), (0, -1), (1, 0))
# 图纸的正面朝北；旋转时按北、东、南、西的顺序跟着转。
FACINGS = ("north", "east", "south", "west")

# 等级 → 最大边长（方块）。规模约束只在插件侧校验，宿主不解释建筑语义。
LEVEL_LIMITS = {"small": 9, "medium": 17, "large": 33, "custom": 33}
DEFAULT_LEVEL = "medium"

SUBMIT_TOOL = "submit_building_blueprint"

SUBMIT_SPEC = {
    "tool": SUBMIT_TOOL,
    "description": (
        "提交最终的建筑蓝图。整个任务只调用一次；提交后任务结束。"
        "蓝图必须是一个 object，能放进本次规模限制内。"
    ),
    "parameters": {
        "type": "object",
        "properties": {
            "summary": {
                "type": "string",
                "description": "一句中文摘要：建的是什么、多大（不超过 120 字）",
            },
            "blueprint": {
                "type": "object",
                "description": "完整蓝图。坐标以锚点为原点，y 向上；内部结构按任务说明。",
            },
        },
        "required": ["summary", "blueprint"],
    },
    "result_field": "blueprint",
    "summary_field": "summary",
}

# 锁锚点时问一次她的位置。本机回环，正常是毫秒级；游戏暂停时别让工具干等。
ANCHOR_TIMEOUT_MS = 3000

# 建筑描述与现场文本的字符上限，避免把导演的上下文撑爆。
PROMPT_CHAR_LIMIT = 2000
SCENE_CHAR_LIMIT = 24000


def _plan_building(ctx):
    """把玩家想建的东西交给导演；立刻返回，不等蓝图。"""
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    args = ctx.get("args") or {}
    prompt = args.get("prompt")
    prompt = prompt.strip() if isinstance(prompt, str) else ""
    if not prompt:
        return {"ok": False, "error": "要说清楚想建什么"}
    prompt = prompt[:PROMPT_CHAR_LIMIT]

    scale, error = _parse_scale(args)
    if error:
        return {"ok": False, "error": error}

    context, error = _build_context(maid)
    if error:
        return {"ok": False, "error": error}

    result = director_task_start(
        "building",
        {
            "system_prompt": _system_prompt(scale),
            "prompt": _request_prompt(prompt, scale, context),
            "include_recent": True,
            "submit": SUBMIT_SPEC,
        },
    )
    if not result.get("ok"):
        return {"ok": False, "error": result.get("error") or "导演任务没能开始"}

    maid_name, player_name = names()
    return {
        "ok": True,
        "message": (
            f"{maid_name}把{player_name}想要的建筑告诉了导演，蓝图还在设计。"
            "设计完会告诉她。"
        ),
    }


def _building_status(_ctx):
    """查当前建筑任务：施工进行到哪，或者导演还在不在设计。"""
    progress = _build_progress()
    if progress:
        return {"ok": True, "message": progress}

    result = director_task_status()
    if not result.get("ok"):
        return {"ok": False, "error": result.get("error") or "查不到导演任务的进度"}

    task = result.get("task")
    _, player_name = names()
    if not isinstance(task, dict):
        return {"ok": True, "message": "现在没有正在设计的建筑。"}

    status = (task.get("status") or "").strip()
    if status == "running":
        return {"ok": True, "message": f"导演还在为{player_name}设计那座建筑。"}
    if status == "done":
        summary = (task.get("summary") or "").strip()
        if summary:
            return {"ok": True, "message": f"导演已经设计好了：{summary}。"}
        return {"ok": True, "message": "导演已经设计好了。"}
    if status == "failed":
        error = (task.get("error") or "").strip()
        return {
            "ok": True,
            "message": f"上一次设计没能完成：{error or '导演没有给出原因'}。",
        }
    return {"ok": True, "message": f"导演任务的状态是{status or '未知'}。"}


def _build_progress():
    """模组那边的施工进度；没在施工、没盖过或老模组认不出这一帧时返回空串。"""
    response, error = ask({"type": "maid_build_status", "data": {}}, STATE_TIMEOUT_MS)
    if error or response.get("type") != "maid_build_status_result":
        return ""
    data = response.get("data") or {}
    status = (data.get("status") or "").strip()
    name = (data.get("name") or "").strip() or HOUSE_NAME
    placed = as_int(data.get("placed"), 0)
    total = as_int(data.get("total"), 0)
    maid, _ = names()
    if status == "running":
        return f"{maid}还在盖{name}，已经放了{placed}/{total}块。"
    if status == "done":
        return f"上一座{name}已经盖好了。"
    if status == "partial":
        return f"上一座{name}盖了一部分，放了{placed}/{total}块，还没盖完。"
    if status == "failed":
        return f"上一座{name}没能盖成。"
    return ""


def _build_house(ctx):
    """用固定图纸盖一座小木屋；立刻返回，施工在模组里按 tick 推进。"""
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    pose, error = _maid_pose(maid)
    if error:
        return {"ok": False, "error": error}

    steps = _facing_step(pose["yaw"])
    frame, error = _send_build(maid, HOUSE_NAME, _front_anchor(pose, steps), _small_house(steps))
    if error:
        return {"ok": False, "error": error}

    maid_name, _ = names()
    if frame.get("resumed"):
        return {
            "ok": True,
            "message": (
                f"{maid_name}回到之前没盖完的工地接着盖{HOUSE_NAME}了；"
                f"缺材料或者盖好了，{maid_name}会开口。"
            ),
        }
    return {
        "ok": True,
        "message": (
            f"{maid_name}拿着图纸开始盖{HOUSE_NAME}了，一共{as_int(frame.get('cells'), 0)}块；"
            f"缺材料或者盖好了，{maid_name}会开口。"
        ),
    }


def _build_climb_test(ctx):
    """施工器的垫脚测试：在面前立一根 2×2、8 格高的柱子，顶上两层只能垫上去。"""
    maid, error = ready(ctx)
    if not maid:
        return {"ok": False, "error": error}

    pose, error = _maid_pose(maid)
    if error:
        return {"ok": False, "error": error}

    steps = _facing_step(pose["yaw"])
    low = -(CLIMB_SIDE // 2)
    cells = [
        {"x": low + x, "y": y, "z": low + z, "block": CLIMB_BLOCK}
        for y in range(CLIMB_HEIGHT)
        for x in range(CLIMB_SIDE)
        for z in range(CLIMB_SIDE)
    ]
    frame, error = _send_build(maid, CLIMB_NAME, _front_anchor(pose, steps), cells)
    if error:
        return {"ok": False, "error": error}

    maid_name, _ = names()
    if frame.get("resumed"):
        return {"ok": True, "message": f"{maid_name}回到上次没盖完的{CLIMB_NAME}接着垫去了。"}
    return {
        "ok": True,
        "message": (
            f"{maid_name}在她面前立一根{as_int(frame.get('cells'), 0)}块的柱子，"
            f"顶上两层得自己垫上去；盖好或卡住她会开口。"
        ),
    }


def _front_anchor(pose, steps):
    """她正前方几格的中心，作为图纸锚点。"""
    forward_x, forward_z = FORWARD[steps]
    return {
        "x": _floor(pose["x"] + forward_x * ANCHOR_DISTANCE + 0.5),
        "y": _floor(pose["y"]),
        "z": _floor(pose["z"] + forward_z * ANCHOR_DISTANCE + 0.5),
    }


def _send_build(maid, name, anchor, cells):
    """把一张图纸交给模组施工。返回 (受理结果, 错误说明)。"""
    response, error = ask(
        {
            "type": BUILD_FRAME,
            "data": {"maid_id": maid, "name": name, "anchor": anchor, "cells": cells},
        },
        ACTION_TIMEOUT_MS,
    )
    if error:
        return None, error
    frame = response.get("data") or {}
    if response.get("type") == "error":
        message = narrate(frame.get("message"))
        if "不支持" in (message or ""):
            message = "这个模组版本还不支持盖房子"
        return None, message or "游戏那边没能开始盖"
    if not frame.get("accepted"):
        return None, narrate(frame.get("reason")) or "游戏拒绝了这次施工"
    return frame, ""


def _maid_pose(maid):
    """问她此刻站在哪、朝哪边。返回 (位置与朝向, 错误说明)。"""
    response, error = ask({"type": "maid_state", "data": {"maid_id": maid}}, ANCHOR_TIMEOUT_MS)
    if error:
        return None, error
    if response.get("type") == "error":
        message = narrate((response.get("data") or {}).get("message"))
        return None, message or "游戏那边没能回答她在哪"
    payload = response.get("data") or {}
    if payload.get("known") and (payload.get("dead") or payload.get("unconscious")):
        return None, "她现在不省人事，没法盖房"
    position = payload.get("position")
    if not isinstance(position, dict) or any(key not in position for key in ("x", "y", "z")):
        return None, "模组没认出这只女仆（UUID 可能变了），或者游戏里装的是旧版模组，没法定施工起点"
    return {
        "x": float(position["x"]),
        "y": float(position["y"]),
        "z": float(position["z"]),
        "yaw": float(position.get("yaw") or 0.0),
    }, ""


def _floor(value):
    """向下取整。内嵌解释器没有 math 模块，int() 又是向零截断，只能自己补负数这一档。"""
    whole = int(value)
    return whole - 1 if value < whole else whole


def _facing_step(yaw):
    """原版 yaw → 0 南、1 西、2 北、3 东。"""
    return _floor(yaw / 90.0 + 0.5) % 4


def _is_window(x, z, y):
    """两格高的窗：正面两对、背面两对、两侧各两对，中柱与门框保持实墙。"""
    if y not in (2, 3):
        return False
    if z == 0:
        return x in (2, 3, 7, 8)
    if z == HOUSE_SIDE - 1:
        return x in (3, 4, 6, 7)
    if x in (0, HOUSE_SIDE - 1):
        return z in (3, 4, 6, 7)
    return False


def _small_house(steps):
    """固定的中型木屋：11×11，四层墙高、外挑一格的高屋顶，带门廊、阁楼感与烟囱。

    材料统一成石砖、云杉木板与台阶、橡木原木与栅栏、玻璃板、火把和灯笼。
    坐标先用 0 到 10 的平面格子（z=0 是带门的正面），最后按她的朝向旋转、平移到中心。
    门的上半格由原版的放置逻辑一起放出来，所以图纸里只写下半格。
    """
    half = HOUSE_SIDE // 2
    last = HOUSE_SIDE - 1
    plan = {}

    def put(x, y, z, block, facing=""):
        plan[(x, y, z)] = (block, facing)

    # 地板：外圈石砖，屋里铺云杉木板；门前铺一小块门廊和一级台阶
    for z in range(HOUSE_SIDE):
        for x in range(HOUSE_SIDE):
            if x in (0, last) or z in (0, last):
                put(x, 0, z, "minecraft:stone_bricks")
            else:
                put(x, 0, z, "minecraft:spruce_planks")
    for x in range(half - 2, half + 3):
        put(x, 0, -1, "minecraft:spruce_planks")
    for x in range(half - 1, half + 2):
        put(x, 0, -2, "minecraft:spruce_stairs", "south")

    # 墙：四层，四角与三面中柱是橡木原木；底层石砖，上面云杉木板，窗开两格高
    for y in (1, 2, 3, 4):
        for z in range(HOUSE_SIDE):
            for x in range(HOUSE_SIDE):
                if x not in (0, last) and z not in (0, last):
                    continue
                if x in (0, last) and z in (0, last):
                    block = "minecraft:oak_log"
                elif x == half and z == 0 and y == 1:
                    block = "minecraft:oak_door"
                elif x == half and z == 0 and y == 2:
                    # 门的上半格跟随下半格一起放，不能单独立项
                    continue
                elif (x in (0, last) and z == half) or (x == half and z == last):
                    block = "minecraft:oak_log"
                elif x in (half - 1, half + 1) and z == 0 and y in (1, 2):
                    block = "minecraft:oak_log"
                elif y == 1:
                    block = "minecraft:stone_bricks"
                elif _is_window(x, z, y):
                    block = "minecraft:glass_pane"
                else:
                    block = "minecraft:spruce_planks"
                put(x, y, z, block, "north" if block == "minecraft:oak_door" else "")

    # 屋顶：从墙顶往外挑一格，之后每层向内收一格；外圈台阶，里面铺木板
    for y, edge in ((5, 6), (6, 5), (7, 4), (8, 3), (9, 2), (10, 1)):
        for z in range(half - edge, half + edge + 1):
            for x in range(half - edge, half + edge + 1):
                if abs(x - half) != edge and abs(z - half) != edge:
                    put(x, y, z, "minecraft:spruce_planks")
                    continue
                if z == half - edge:
                    facing = "south"
                elif z == half + edge:
                    facing = "north"
                elif x == half - edge:
                    facing = "east"
                else:
                    facing = "west"
                put(x, y, z, "minecraft:spruce_stairs", facing)
    # 顶脊压一块半砖
    put(half, 11, half, "minecraft:spruce_slab")
    # 门廊屋檐下留两块实心木板挂灯笼，其余外圈保持台阶
    for x in (half - 1, half + 1):
        put(x, 5, -1, "minecraft:spruce_planks")

    # 烟囱：贴在后墙偏东，从地基一路穿出屋顶（里面不放火）
    for y in range(0, 10):
        put(last - 2, y, last, "minecraft:stone_bricks")
    put(last - 2, 10, last, "minecraft:stone_brick_slab")

    # 门廊：两根原木柱撑住屋檐，中间留一格宽的进出口，柱间安栅栏
    for x in (half - 2, half + 2):
        for y in (1, 2, 3, 4):
            put(x, y, -1, "minecraft:oak_log")
    for x in (half - 1, half + 1):
        put(x, 1, -1, "minecraft:oak_fence")

    # 火把：两面山墙各挂两支；灯笼：门廊屋檐下悬两盏
    for x in (1, last - 1):
        put(x, 3, -1, "minecraft:torch")
        put(x, 3, last + 1, "minecraft:torch")
    for x in (half - 1, half + 1):
        put(x, 4, -1, "minecraft:lantern")

    cells = [(x, y, z, block, facing) for (x, y, z), (block, facing) in plan.items()]
    cells.sort(key=lambda cell: (cell[1], cell[2], cell[0]))
    return [_rotate_cell(cell, steps) for cell in cells]


def _rotate_cell(cell, steps):
    """把正面朝北的图纸转到她面朝的那一边，并换成以中心为原点的偏移。"""
    x, y, z, block, facing = cell
    last = HOUSE_SIDE - 1
    if steps == 0:
        rx, rz = x, z
    elif steps == 1:
        rx, rz = last - z, x
    elif steps == 2:
        rx, rz = last - x, last - z
    else:
        rx, rz = z, last - x
    out = {"x": rx - HOUSE_SIDE // 2, "y": y, "z": rz - HOUSE_SIDE // 2, "block": block}
    if facing:
        out["facing"] = FACINGS[(FACINGS.index(facing) + steps) % 4]
    return out


def _system_prompt(scale):
    """导演的任务说明。建筑领域的约束都在这里，宿主不参与。"""
    level = scale["level"]
    width = scale["width"]
    height = scale["height"]
    depth = scale["depth"]
    return (
        "你是 LingChat 的建筑导演，负责把玩家想要的东西设计成一份可以照着施工的蓝图。\n"
        "你只做设计，不替角色说话，也不直接改世界。\n"
        "\n"
        "【本次限制】\n"
        f"规模等级：{level}；宽×高×深不得超过 {width}×{height}×{depth} 格。\n"
        "整座建筑的目标方块数不超过 4000；超过时先缩小规模或简化结构。\n"
        "蓝图坐标以锚点为原点，x 向东、y 向上、z 向南，所有方块必须落在限制范围内。\n"
        "\n"
        "【蓝图要求】\n"
        "- 先保证结构完整、能站人、有出入口，再考虑装饰。\n"
        "- 尽量使用她背包里已经有的方块；材料不足时可以写进蓝图说明，但不要假设会自动补货。\n"
        "- 不要使用岩浆、火、TNT 这类会破坏现场的方块。\n"
        "- 现场地形与最近对话在用户消息里；地形不合适时让结构去适应它，不要要求玩家先改造。\n"
        "- 蓝图内部格式本期不锁死，但必须是一个 object；把尺寸、材料与结构说明写进去，"
        "方便后续施工器读取。\n"
        "\n"
        f"【提交】\n设计完成后调用 {SUBMIT_TOOL}，整个任务只调用一次。"
        "summary 用一句中文说明建的是什么、多大。"
    )


def _request_prompt(prompt, scale, context):
    """本次任务的用户消息：愿望、规模与现场。最近对话由宿主追加。"""
    level = scale["level"]
    width = scale["width"]
    height = scale["height"]
    depth = scale["depth"]
    scene = json.dumps(context, ensure_ascii=False, separators=(",", ":"))
    if len(scene) > SCENE_CHAR_LIMIT:
        scene = scene[:SCENE_CHAR_LIMIT] + "……（现场信息过长，后半段省略）"
    return (
        f"【建筑愿望】\n{prompt}\n\n"
        f"【规模】\n{level}，上限 {width}×{height}×{depth} 格；本次按这个范围设计。\n\n"
        f"【现场（由桥接插件提供，字段原样）】\n{scene}"
    )


def _parse_scale(args):
    """把工具的扁平参数收成请求里的 scale 对象。返回 (对象, 错误说明)。"""
    raw = args.get("scale")
    level = raw.strip().lower() if isinstance(raw, str) else ""
    if level not in LEVEL_LIMITS:
        level = DEFAULT_LEVEL
    limit = LEVEL_LIMITS[level]

    dims = {}
    for key in ("width", "height", "depth"):
        value = args.get(key)
        if value is None or value == "":
            dims[key] = None
            continue
        number = as_int(value, 0)
        if number <= 0:
            return None, f"{key} 必须是正整数"
        if number > limit:
            return None, f"{level} 规模的 {key} 不能超过 {limit}（收到 {number}）"
        dims[key] = number

    if level == "custom":
        missing = [key for key, value in dims.items() if value is None]
        if missing:
            return None, "custom 规模需要同时给出 width、height、depth"
    for key, value in dims.items():
        if value is None:
            dims[key] = limit
    return {"level": level, **dims}, ""


def _build_context(maid):
    """给导演的现场：锚点、地形快照与背包。锚点取发起这一刻的她。"""
    data, error = snapshot(maid, LOOK_TIMEOUT_MS, "query", "map")
    if error:
        return None, error

    state, state_error = ask({"type": "maid_state", "data": {}}, ANCHOR_TIMEOUT_MS)
    anchor = None
    dimension = data.get("dimension") or ""
    if not state_error and state.get("type") == "maid_state_result":
        payload = state.get("data") or {}
        if payload.get("known") and (payload.get("dead") or payload.get("unconscious")):
            return None, "她现在不省人事，没法锁定施工起点"
        anchor = payload.get("position")
        dimension = payload.get("dimension") or dimension
    if not isinstance(anchor, dict):
        return None, "模组没认出这只女仆（UUID 可能变了），或者游戏里装的是旧版模组，没法锁定施工起点"

    return {
        "dimension": dimension,
        "anchor": {key: anchor.get(key) for key in ("x", "y", "z")},
        "terrain": {
            "standing_on": data.get("standing_on"),
            "overhead": data.get("overhead"),
            "structure": data.get("structure"),
            "relations": _limit(data.get("relations"), 12),
            "clusters": _limit(data.get("clusters"), 12),
            "blocks": _limit(data.get("blocks"), 24),
            "block_sight": _limit(data.get("block_sight"), 24),
            "map": _map_text(data),
        },
        "inventory": _limit(data.get("backpack"), 36),
    }, ""


def _limit(value, count):
    return value[:count] if isinstance(value, list) else []


def _map_text(data):
    info = data.get("map")
    text = info.get("text") if isinstance(info, dict) else ""
    text = text.strip("\n") if isinstance(text, str) else ""
    return text[:2000]


TOOLS = {
    PLAN_TOOL: _plan_building,
    STATUS_TOOL: _building_status,
    BUILD_TOOL: _build_house,
    CLIMB_TOOL: _build_climb_test,
}
