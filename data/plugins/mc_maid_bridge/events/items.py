"""物品的来去：她捡起来的、背包里多出来或少了、喂她吃、她自己吃，以及主人拿出她最爱吃的。"""

from events.spec import EventSpec
from naming import names
from wording import BLOCK_NAMES, ITEM_NAMES, MOB_NAMES, as_int, item_name, name_of


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

# 拿走时的说法，与上面那组对称
SLOT_TAKE_PHRASES = {
    "hand": "把{maid}手里的{items}拿走了",
    "armor": "把{maid}身上的{items}脱了下来",
    "bauble": "把{maid}戴着的{items}取了下来",
}
PACK_TAKE = "从{maid}的背包里拿走了{items}"


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


def item_taken(data):
    """她身上少了东西：玩家在女仆界面里拿走了。

    模组只在主人贴身（8 格内）、她自己又没在干活时报——不然农场播种、拉弓射箭吃掉的那些
    也会算成被人拿走。她自己吃掉与交给主人的那几件事先记过账，也走不到这里。

    少了哪儿由模组按格子差分判（`slot`），说法与「放进来」那组对称。
    """
    maid, player = names()
    clauses = []
    for slot, items in _by_slot(data.get("items")):
        phrase = SLOT_TAKE_PHRASES.get(slot) or PACK_TAKE
        clauses.append(phrase.format(maid=maid, items=items))
    if not clauses:
        return f"{player}从{maid}的背包里拿走了{_item_list(data.get('items'))}。"
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
    """玩家喂她吃了东西（潜行右键食物）。连喂几口由模组合成一条。

    带汤带碗的食物吃完会剩下容器，模组把那一份也放在 `leftover` 里；碗是车万女仆
    自己收进她背包的，旁白里顺带说一句，免得模型对背包里的碗来路不明。
    """
    maid, player = names()
    return f"{player}喂{maid}吃了{_item_list(data.get('items'))}{_leftover(data)}。"


def maid_ate(data):
    """她自己从背包里找东西吃了（车万女仆的进食系统）。"""
    maid, _ = names()
    return f"{maid}自己吃了{_item_list(data.get('items'))}{_leftover(data)}。"


def favorite_food(data):
    """主人手里拿着她最爱吃的东西，她凑过去讨。

    模组只在她真的讨起来（车万女仆的讨食姿态已经翻过去）时推，所以不用再判距离：
    这条说的就是她看得见、也走得到跟前的那一刻。模组食物多半没有中文名，认不出时
    只说「最爱吃的东西」——绝不能把 id 或英文名塞进旁白，模型会照着原样念出来。
    """
    maid, player = names()
    raw = (data.get("item") or "").strip()
    name = ITEM_NAMES.get(raw) or BLOCK_NAMES.get(raw) or ""
    if name:
        return f"{player}手里拿着{maid}最爱吃的{name}，{maid}凑到跟前眼巴巴地看着。"
    return f"{player}手里拿着{maid}最爱吃的东西{raw}（物品id，不要照着念），{maid}凑到跟前眼巴巴地看着。"


def _leftover(data):
    """吃完剩下的容器：有就补一句，没有就空串。"""
    leftover = data.get("leftover") or []
    if not leftover:
        return ""
    return f"，吃完剩下{_item_list(leftover)}"


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
    "item_taken": EventSpec(item_taken, urgent=True, ttl=30),
    "item_fed": EventSpec(item_fed, urgent=True, ttl=30),
    "maid_ate": EventSpec(maid_ate, ttl=60),
    # 和挨打一档：主人把她最爱吃的拿出来了，她这会儿正凑上去讨
    "favorite_food": EventSpec(favorite_food, urgent=True, ttl=60, snapshot=True),
}
