"""旁白里的称呼：当前角色名与玩家名。

旁白一律用第三人称，直接叫名字，不说「你」「玩家」。名字由宿主给出
（`current_names`：角色的 settings.ai_name 与玩家的 user_name），插件不自己猜。
取不到时退回「女仆」「主人」——仍然是第三人称，不会把旁白写成第二人称。
"""

try:
    from plugin_host import current_names
except ImportError:  # 旧宿主没有这个名字接口：照常跑，退回默认称呼
    def current_names():
        return {"ok": False}


FALLBACK_MAID = "女仆"
FALLBACK_PLAYER = "主人"


def names():
    """当前 (角色名, 玩家名)。接口不可用或名字为空时给默认称呼。"""
    info = current_names()
    if not info.get("ok"):
        return FALLBACK_MAID, FALLBACK_PLAYER
    maid = (info.get("ai_name") or "").strip() or FALLBACK_MAID
    player = (info.get("user_name") or "").strip() or FALLBACK_PLAYER
    return maid, player


def narrate(text):
    """把模组回执里的泛指换成本次称呼：玩家 → 名字，你 → 角色名。

    「别的玩家」是分类说法，不是主人，先单独换成「另一个人」再替换。
    模组自己的「她」保持不动——那是第三人称，读起来仍然通顺。
    """
    text = (text or "").strip()
    if not text:
        return text
    maid, player = names()
    text = text.replace("别的玩家", "另一个人").replace("另一个玩家", "另一个人")
    return text.replace("玩家", player).replace("你", maid)
