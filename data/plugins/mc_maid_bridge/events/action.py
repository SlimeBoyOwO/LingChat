"""动作收尾：走过去/坐下/躺下的终态，以及挖掘的终态。"""

from events.spec import EventSpec
from naming import names, narrate
from wording import BLOCK_NAMES, as_int, name_of

# 受理即完成的动作收尾时记进台词表的旁白。go_to 另有说法（看走到哪了）。
ACTION_DONE_PHRASES = {
    "sit": "{maid}坐下了。",
    "stand": "{maid}站起来了。",
    "lie_down": "{maid}躺到床上睡着了。",
}


def action_done(data):
    """动作收尾。三种结局都要说：被顶掉也要说，否则角色会一直以为还在做。"""
    maid, player = names()
    action = (data.get("action") or "").strip()
    target = (data.get("target") or "").strip()
    status = (data.get("status") or "").strip()
    ground_lie = action == "lie_down" and target == "ground"
    if status == "done":
        if action == "go_to":
            if target == "bed":
                return f"{maid}走到了床边。"
            return f"{maid}走到了{player}身边。"
        if ground_lie:
            # 就地躺下的收尾不是「办成了」，而是她自己爬了起来：原因里写着是什么打断的
            reason = narrate(data.get("reason"))
            return f"{maid}没再躺着：{reason}。" if reason else f"{maid}自己爬了起来。"
        return ACTION_DONE_PHRASES.get(action, "{maid}这件事做完了。").format(maid=maid)
    if status == "cancelled":
        if ground_lie:
            return f"{maid}没再躺着，被新的事叫起来了。"
        return f"{maid}手上那件事被新的事顶掉了，没做成。"
    reason = narrate(data.get("reason"))
    return f"{maid}刚才那件事没做成：{reason}" if reason else f"{maid}刚才那件事没做成。"


def maid_mine_done(data):
    """挖掘收尾：挖到多少、还差多少、为什么停。

    工具受理只代表开始挖了，模型很容易把「正过去挖」当成「已经挖到了」；这条旁白才是
    唯一能让它改口的依据。走不到、工具不对也在这里说清楚，不留悬案。
    """
    maid, _ = names()
    name = name_of(BLOCK_NAMES, data.get("block"), "方块")
    mined = as_int(data.get("mined"), 0)
    wanted = as_int(data.get("wanted"), mined)
    status = (data.get("status") or "").strip()
    if status == "done":
        return f"{maid}挖掉了一块{name}。" if mined == 1 else f"{maid}挖掉了{mined}块{name}。"
    reason = narrate(data.get("reason"))
    if mined > 0:
        text = f"{maid}挖掉了{mined}块{name}"
        if wanted > mined:
            text += f"，还差{wanted - mined}块没挖完"
        return text + (f"：{reason}。" if reason else "。")
    return f"{maid}没能挖成：{reason}。" if reason else f"{maid}没能挖成。"


SPECS = {
    "action_done": EventSpec(action_done, urgent=True, ttl=60, snapshot=True),
    "maid_mine_done": EventSpec(maid_mine_done, urgent=True, ttl=120),
}
