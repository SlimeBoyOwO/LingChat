"""玩家侧的事件：拿到成就，以及女仆自己换了工作模式。"""

from events.spec import EventSpec
from naming import names
from wording import advancement_name, task_name


def player_advancement(data):
    """玩家拿到了成就（原版叫「进度」）。挑战类另说一句，语气不一样。"""
    _, player = names()
    name = advancement_name(data.get("advancement"), data.get("title"))
    if (data.get("frame") or "") == "challenge":
        return f"{player}完成了挑战「{name}」。"
    return f"{player}获得了成就「{name}」。"


def maid_task_change(data):
    """她自己换了工作模式（农场、钓鱼、空闲……）。

    只说换成了什么，不催她开工：她现在会不会去干、在哪儿干，工具那次回执里已经说过了，
    而这条旁白要留在台词表里，管的是过一会儿她还记不记得自己在干什么。
    """
    maid, _ = names()
    return f"{maid}换成了工作模式「{task_name(data.get('task'), data.get('summary'))}」。"


SPECS = {
    "player_advancement": EventSpec(player_advancement, ttl=600),
    "maid_task_change": EventSpec(maid_task_change, ttl=300),
}
