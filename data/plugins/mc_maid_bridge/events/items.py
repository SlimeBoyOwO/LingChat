"""物品的来去：丢给她、她捡起来、背包里多出来、喂她吃、她自己吃。"""

from events.spec import EventSpec
from naming import names
from wording import as_int, item_name


def item_gift(data):
    """主人把手里的东西丢在她身边。模组把一小段连续投掷合成一条。

    只记「给了什么」，不写数字之外的推测。她未必会去捡（拾物模式没开、背包没格就捡不了），
    捡没捡另有 `item_pickup` 那条。
    """
    maid, player = names()
    place = "脚边" if _close(data.get("distance")) else "身边"
    return f"{player}往{maid}{place}丢下了{_item_list(data.get('items'))}。"


def item_pickup(data):
    """她把地上的东西收进了背包。玩家丢的那份值得当场开口，其余只是记一笔。"""
    maid, player = names()
    items = _item_list([data])
    if data.get("from_owner"):
        return f"{maid}把{player}丢下的{items} 收进了背包。"
    return f"{maid}捡起了{items}。"


def item_received(data):
    """背包里多出东西，多半是玩家在女仆界面里放进去的。界面那条路没有事件可挂，
    模组只能前后比一次，所以这里只说「多了什么」，不硬认是谁放的。"""
    maid, _ = names()
    return f"{maid}的背包里多了{_item_list(data.get('items'))}。"


def item_fed(data):
    """玩家喂她吃了东西（潜行右键食物）。连喂几口由模组合成一条。"""
    maid, player = names()
    return f"{player}喂{maid}吃了{_item_list(data.get('items'))}。"


def maid_ate(data):
    """她自己从背包里找东西吃了（车万女仆的进食系统）。"""
    maid, _ = names()
    return f"{maid}自己吃了{_item_list(data.get('items'))}。"


def _item_list(items):
    """[{item,count}, ...] → 「熟牛肉 ×2、面包」。不认识的 id 退回英文后半段，模型也认。"""
    parts = []
    for one in items or []:
        if not isinstance(one, dict):
            continue
        name = item_name(one.get("item"))
        count = as_int(one.get("count"), 1)
        parts.append(f"{name} ×{count}" if count > 1 else name)
    return "、".join(parts) or "东西"


def _close(distance):
    """丢的东西是不是就在脚边（三格内）。给不出距离就当脚边。"""
    try:
        return float(distance) <= 3.0
    except (TypeError, ValueError):
        return True


SPECS = {
    "item_gift": EventSpec(item_gift, ttl=60),
    "item_pickup": EventSpec(
        item_pickup,
        ttl=5,
        urgent_when=lambda data: bool(data.get("from_owner")),
    ),
    "item_received": EventSpec(item_received, urgent=True, ttl=30),
    "item_fed": EventSpec(item_fed, urgent=True, ttl=30),
    "maid_ate": EventSpec(maid_ate, ttl=60),
}
