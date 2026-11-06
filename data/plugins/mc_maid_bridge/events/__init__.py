"""游戏事件 → 中文事实句与元数据。

每类事件的格式器、紧急度、TTL、合并键、是否附带现场快照都在各自模块里登记；
这里只做汇总与对外 API。认不出的 event_type 一律返回空文本，保持向后兼容。
"""

from events.action import SPECS as ACTION_SPECS
from events.carry import SPECS as CARRY_SPECS
from events.combat import SPECS as COMBAT_SPECS
from events.items import SPECS as ITEM_SPECS
from events.player import SPECS as PLAYER_SPECS
from events.spec import DEFAULT_EVENT_TTL
from events.world import SPECS as WORLD_SPECS

SPECS = {}
for _part in (WORLD_SPECS, COMBAT_SPECS, ITEM_SPECS, CARRY_SPECS, ACTION_SPECS, PLAYER_SPECS):
    SPECS.update(_part)


def spec_for(kind):
    return SPECS.get(kind)


def format_event(data):
    """一条事件 → (中文, 是否紧急)。认不出的类型返回空文本，调用方据此什么都不做。"""
    kind = (data.get("event_type") or "").strip()
    spec = SPECS.get(kind)
    if spec is None:
        return "", False
    return spec.format(data), spec.is_urgent(data)


def event_ttl(kind, data=None):
    """这类事件值多少秒；登记成函数的（受伤分档）要看数据。"""
    spec = SPECS.get(kind)
    return spec.ttl_of(data or {}) if spec else DEFAULT_EVENT_TTL


def event_key(kind):
    """同类事件在队列里的合并键；不需要合并的返回空串。"""
    spec = SPECS.get(kind)
    return spec.merge_key if spec else ""


def event_severity(kind, data):
    """同键合并时比大小的严重度。"""
    spec = SPECS.get(kind)
    return spec.severity_of(data) if spec else 0


def needs_snapshot(kind):
    """这类事件发生时，要不要顺手再取一份她身边的现场快照。"""
    spec = SPECS.get(kind)
    return bool(spec and spec.snapshot)


def snapshot(data):
    """接上世界时的第一帧：把眼下的处境交代清楚。"""
    return SPECS["state_snapshot"].format(data)
