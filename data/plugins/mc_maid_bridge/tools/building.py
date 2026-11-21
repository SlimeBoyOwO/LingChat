"""建筑工具：把建筑愿望交给导演生成蓝图，以及查生成进度。

建筑提示词与提交工具 schema 都由本插件组装，宿主只跑通用的导演任务循环。蓝图不回到
女仆这里：导演完成后由宿主的 director_task 信号通知插件，插件再按任务 id 取走；转给
模组施工是下一期。工具只负责投递与查进度，不阻塞当前对话。
"""

import json

from plugin_host import director_task_start, director_task_status

from bridge import ask, ready
from naming import names
from tools.common import LOOK_TIMEOUT_MS
from tools.context import snapshot
from wording import as_int

PLAN_TOOL = "maid_plan_building"
STATUS_TOOL = "maid_building_status"

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
    """查当前建筑任务：生成中 / 已完成 / 失败。"""
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
        return None, "拿不到她在世界里的位置，没法锁定施工起点"

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
}
