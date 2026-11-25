package com.lingchat.maidbridge.action.nav;

/** 寻路失败的原因：调用方拿它记格子状态，不再自己判是哪一种走不通。 */
public enum NavFailure {
    /** 旁边没有能站的地方。 */
    NO_STAND,
    /** 有地方站，但走不过去。 */
    NO_PATH,
    /** 她待的地方头顶太低，跳不起来也垫不上去。 */
    NO_HEADROOM,
    /** 站到了也够不着目标。 */
    OUT_OF_REACH,
    /** 这一次寻路拖得太久。 */
    TIMEOUT,
    /** 搜索被节点额度截断，没算完：不能当成没有路。 */
    SEARCH_LIMIT
}
