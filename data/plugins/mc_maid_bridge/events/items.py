"""物品的来去：她捡起来的东西、背包里多出来、喂她吃、她自己吃。"""

from events.spec import EventSpec
from naming import names
from wording import BLOCK_NAMES, MOB_NAMES, as_int, item_name, name_of


def item_pickup(data):
    """她把地上的东西收进了背包。只在她真的捡起来时报，并说清来源。

    地上的东西不单独报：主人随手丢下、怪物掉的、挖方块崩出来的，她没捡就与她无关。
    来源由模组认：主人 / 别的玩家（带名字）/ 哪只生物掉的 / 挖哪种方块崩出来的。
    """
    maid, player = names()
    items = _item_list([data])
    source = (data.get("source") or "").strip()
    if source == "owner":
        return f"{maid}把{player}丢下的{items}收进了背包。"
    if source == "player":
        who = (data.get("source_name") or "").strip() or "另一个玩家"
        return f"{maid}把{who}丢下的{items}收进了背包。"
    if source == "mob":
        who = name_of(MOB_NAMES, data.get("source_id"), "什么生物")
        return f"{maid}捡起了{who}掉的{items}。"
    if source == "block":
        block = name_of(BLOCK_NAMES, data.get("source_id"), "方块")
        return f"{maid}捡起了挖{block}时崩出来的{items}。"
    return f"{maid}捡起了{items}。"


# 东西放在她身上的哪儿：手、盔甲栏、饰品栏、背包。模组按格子差分判的。
SLOT_PHRASES = {
    "hand": "把{items}递到了{maid}手里",
    "armor": "给{maid}穿上了{items}",
    "bauble": "给{maid}戴上了{items}",
}
PACK_BACKPACK = "往{maid}的背包里放了{items}"


def item_received(data):
    """她身上多出东西。界面那条路没有事件可挂，模组只能前后比一次。

    主人在旁边（模组判的 `from_owner`）就按「主人放进来的」说：女仆界面必须贴身才
    打得开，这个距离内没有第二条路能把东西塞进去。主人不在旁边时只说「多了什么」，
    不替别人认领。

    放在哪儿由模组判（`slot`）：塞进手里、穿在身上、挂到饰品栏，说法都不一样——
    原来一律说「往背包里放了」，玩家把铁镐放进她主手时她对不上号。
    """
    maid, player = names()
    groups = _by_slot(data.get("items"))
    if not data.get("from_owner"):
        return f"{maid}身上多了{_item_list(data.get('items'))}。"
    clauses = []
    for slot, items in groups:
        phrase = SLOT_PHRASES.get(slot) or PACK_BACKPACK
        clauses.append(phrase.format(maid=maid, items=items))
    if not clauses:
        return f"{maid}身上多了{_item_list(data.get('items'))}。"
    return f"{player}{'，'.join(clauses)}。"


def _by_slot(items):
    """[(slot, 物品串), ...]：同一处的合成一句，顺序按第一次出现的先后。"""
    order = []
    grouped = {}
    for one in items or []:
        if not isinstance(one, dict):
            continue
        slot = (one.get("slot") or "backpack").strip() or "backpack"
        if slot not in grouped:
            grouped[slot] = []
            order.append(slot)
        grouped[slot].append(one)
    return [(slot, _item_list(grouped[slot])) for slot in order]


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


SPECS = {
    "item_pickup": EventSpec(
        item_pickup,
        ttl=5,
        urgent_when=lambda data: bool(data.get("from_owner")),
    ),
    "item_received": EventSpec(item_received, urgent=True, ttl=30),
    "item_fed": EventSpec(item_fed, urgent=True, ttl=30),
    "maid_ate": EventSpec(maid_ate, ttl=60),
}
