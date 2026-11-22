package com.lingchat.maidbridge.action.construction;

/**
 * 跑跳的几何规则：冲刺跳能跨多远、抬高之后还剩多远。
 *
 * <p>与玩家一致：平地最远四格；往上抬一格只剩三格，再高跳不上去；往下最多落三格，
 * 横向不再增加。规则不碰世界，能脱开 Minecraft 跑单测。
 */
public final class ParkourRules {
    /** 平地的最大横向距离，按起跳格到落点格算。 */
    public static final int MAX_DISTANCE = 4;
    /** 抬高时缩短到三格：跳起来只有那么点前冲。 */
    public static final int UP_DISTANCE = 3;
    /** 往上最多抬一格。 */
    public static final int MAX_RISE = 1;
    /** 往下最多落三格：再深就可能摔伤，也不该往那边跳。 */
    public static final int MAX_FALL = 3;
    /** 至少隔两格才叫跑跳：相邻一格交给普通走路和跳上一格。 */
    public static final int MIN_DISTANCE = 2;
    /** 平地这一跳大约飞多少 tick，用来折算横向速度。 */
    public static final int FLIGHT_TICKS = 10;

    private ParkourRules() {
    }

    /** 这一跳允许吗：距离与高差同时满足才有意义。 */
    public static boolean allowed(int distance, int rise) {
        if (distance < MIN_DISTANCE || rise > MAX_RISE || rise < -MAX_FALL) {
            return false;
        }
        return distance <= (rise > 0 ? UP_DISTANCE : MAX_DISTANCE);
    }

    /** 这一跳要飞多少 tick：往下掉得越多，空中时间越长。 */
    public static int flightTicks(int rise) {
        return FLIGHT_TICKS + Math.max(0, -rise) * 2;
    }
}
