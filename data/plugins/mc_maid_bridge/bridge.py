"""与模组之间的通道：连没连上、绑的是哪只女仆、发一条帧、等一个回信。

插件脚本每次调用都新建解释器、跑完即弃，没有跨调用状态，所以「现在连上了吗」「绑的是
哪只女仆」一律当场问；要长期记住的东西（地址、女仆 UUID、玩家名）都放在配置项里。
`plugin_host` 的 ws_* 只有这里 import：别处要用连接，都从这几个函数走。
"""

from plugin_host import ws_open, ws_request, ws_send, ws_status

from wording import as_int

CONN_ID = "tlm"
DECLARED_WS_URL = "ws://127.0.0.1:48920"

# 告诉模型「游戏那边没接上」时的统一说法，别处不要再造一句同义的。
OFFLINE = "还没连上 Minecraft，做不到。"
NO_REPLY = "游戏没有回应"

# 攒够几条游戏事件、且这期间玩家没说话，就请角色主动开口。0 = 只记旁白，不请。
DEFAULT_PROACTIVE_AFTER = 3

# 配置里没有女仆 UUID 时，现问一次游戏要多久。本机回环，正常是毫秒级。
MAID_QUERY_TIMEOUT_MS = 5000


def config(ctx):
    return ctx.get("config") or {}


def open_connection(url):
    """把配置里的地址应用到声明的那条连接上（只在非默认地址时需要）。"""
    ws_open(CONN_ID, url=url)


def connected():
    status = ws_status(conn_id=CONN_ID)
    return bool(status.get("ok")) and status.get("state") == "connected"


def send(frame):
    """推一条不期待回信的帧，返回宿主的结果（发不出去时 `ok` 为假，别默默丢掉）。"""
    return ws_send(CONN_ID, frame)


def bind(maid_id, owner_name=""):
    """告诉模组盯哪只女仆：玩家说话与「她自发注意到」都得先有这一步。

    模组那边是幂等的：重复发只当又改了一次绑定目标，所以可以放心在别的时机重申。
    顺带把主人的 MC 用户名交过去：她死后实体查不到，模组靠这个名字在复活时认出
    「胶片里的是不是你那位主人」。
    """
    data = {"maid_id": maid_id}
    if owner_name:
        data["owner_name"] = owner_name
    return send({"type": "set_monitored_maid", "data": data})


def ask(frame, timeout_ms):
    """发一条请求帧等回信。返回 (整个响应帧, 错误说明)；出错时响应帧是 None。

    超时要明显小于工具自身的超时，否则模型在工具超时里干等。
    """
    result = ws_request(CONN_ID, frame, timeout_ms=timeout_ms)
    if not result.get("ok"):
        return None, result.get("error") or NO_REPLY
    return result.get("response") or {}, ""


def maid_id(cfg):
    """绑定的女仆 UUID；没配现问一次。返回 (id, 错误说明)。

    插件无状态存不下发现到的 id，所以未配置时每次用到都要重问一遍；填了配置就免掉
    这次往返。建议从模组面板复制填上。游戏里有多只女仆时，优先选 `player_name` 那位
    主人名下的，免得认到别人的女仆。
    """
    configured = cfg.get("maid_id")
    configured = configured.strip() if isinstance(configured, str) else ""
    if configured:
        return configured, ""
    response, error = ask({"type": "get_maid_status", "data": {}}, MAID_QUERY_TIMEOUT_MS)
    if error:
        return "", error
    maids = (response.get("data") or {}).get("maids") or []
    if not maids:
        return "", "游戏里没有找到女仆"
    # 游戏里不止一只时优先认自己那只：主人的账号名配置项就是为这一步准备的。
    owner = player_name(cfg)
    if owner:
        for one in maids:
            if (one.get("owner") or "").strip() == owner:
                return one.get("id") or "", ""
    return maids[0].get("id") or "", ""


def ready(ctx):
    """能干活了吗：连上了、也知道是哪只女仆。返回 (maid_id, 错误说明)。"""
    if not connected():
        return "", OFFLINE
    return maid_id(config(ctx))


def player_name(cfg):
    """玩家在 MC 里的用户名（配置项）。没填就是空串。

    只用来判断「游戏里说话的是不是主人本人」：是就不带账号名，不是才点名。
    事件旁白不再用它——那边一律用 `current_names()` 给的 LingChat 名字。
    """
    raw = cfg.get("player_name")
    return raw.strip() if isinstance(raw, str) else ""


def proactive_after(ctx):
    """攒够几条游戏事件才请角色开口；0 = 只记旁白不请。"""
    raw = config(ctx).get("proactive_after_events")
    if raw is None or raw == "":
        return DEFAULT_PROACTIVE_AFTER
    return max(0, as_int(raw, DEFAULT_PROACTIVE_AFTER))


def truthy(value, default):
    """配置项可能是布尔、也可能是字符串（前端表单传过字符串）。"""
    if value is None:
        return default
    if isinstance(value, str):
        return value.strip().lower() not in ("", "false", "0", "no")
    return bool(value)
