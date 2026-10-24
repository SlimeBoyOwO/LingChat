"""Carry On 的抱起来 / 放下来。"""

from events.spec import EventSpec
from naming import names


def maid_carried(data):
    """被 Carry On 抱了起来。她主人才能抱她（那个模组只允许主人动驯服生物）。"""
    maid, player = names()
    return f"{player}把{maid}抱了起来。"


def maid_dropped(data):
    """抱了一会儿又放下来了。"""
    maid, player = names()
    return f"{player}把{maid}放了下来。"


SPECS = {
    "maid_carried": EventSpec(maid_carried, urgent=True, ttl=30),
    "maid_dropped": EventSpec(maid_dropped, ttl=30),
}
