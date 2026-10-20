"""工具入口：按工具名分派到各行为模块。

manifest 里所有工具都指向这个脚本；每个行为模块自己维护 TOOLS 表，
这里只做汇总与「认不出的工具如实拒绝」。
"""

from tools.action import TOOLS as ACTION_TOOLS
from tools.combat import TOOLS as COMBAT_TOOLS
from tools.item import TOOLS as ITEM_TOOLS
from tools.mine import TOOLS as MINE_TOOLS
from tools.query import TOOLS as QUERY_TOOLS
from tools.task import TOOLS as TASK_TOOLS

TOOLS = {}
for _part in (ACTION_TOOLS, TASK_TOOLS, COMBAT_TOOLS, MINE_TOOLS, ITEM_TOOLS, QUERY_TOOLS):
    TOOLS.update(_part)


def run(ctx):
    """工具入口。认不出的工具如实拒绝，不猜。"""
    tool = (ctx.get("tool_name") or "").strip()
    handler = TOOLS.get(tool)
    if handler is None:
        return {"ok": False, "error": f"这个工具不存在：{tool}"}
    return handler(ctx)
