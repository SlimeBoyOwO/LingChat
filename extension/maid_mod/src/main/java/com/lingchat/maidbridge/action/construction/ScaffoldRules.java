package com.lingchat.maidbridge.action.construction;

/**
 * 垫脚的判据：什么时候还值得往她脚下垫一块，哪一步必须真的接近目标。
 *
 * <p>目标高过脚下两层以上时是往上爬，每垫一格都得离目标更近；目标与她齐平、
 * 或者只高一格时，垫这一格是为了站到放得下的位置，方向不必更近——屋顶边上
 * 常常只有远离屋顶的那一侧还有空位，先垫上去再走回来才是能走的路。
 */
public final class ScaffoldRules {
    /** 一次施工最多垫这么多块：垫脚只是兜底，不能把整座楼都拿去换台阶。 */
    public static final int MAX_BLOCKS = 16;
    /** 同一格最多为它垫几次脚（按升高次数算，不是放下的方块数）。 */
    public static final int MAX_STEPS = 6;
    /** 同一格最多为它平铺几块路：平路一格一块，走不过去的缺口总有走完的时候。 */
    public static final int MAX_BRIDGE = 8;
    /** 目标高过脚下这么多就不垫了，通常是图纸本身写得不对。 */
    public static final int MAX_RISE = 12;
    /** 目标水平离得太远也不垫：先走近，而不是在远处搭一座塔。 */
    public static final int MAX_DISTANCE = 8;

    private ScaffoldRules() {
    }

    /** 目标不低于脚下、也没高过头时还可以垫：齐平与只高一格正是「离屋顶还差一格」那一步。 */
    public static boolean mayStep(int rise) {
        return rise >= 0 && rise <= MAX_RISE;
    }

    /** 往上爬的途中每垫一格都必须比现在更接近目标；齐平或只高一格时先看垫完放不放得下。 */
    public static boolean mustApproach(int rise) {
        return rise >= 2;
    }

    /**
     * 目标与脚下齐平时可以平着搭过去。
     *
     * <p>像起床战争那样在脚下这一层铺一格走过去，人始终同一个高度；高一格以上交给垫脚，
     * 往下的交给跳。
     */
    public static boolean mayBridge(int rise) {
        return rise == 0;
    }

    /** 平着搭路先走差得多的那条轴；两个方向一样远时先横着。 */
    public static boolean alongX(int dx, int dz) {
        return Math.abs(dx) >= Math.abs(dz);
    }

    /** 目标水平方向在够得着的垫脚范围里（每个轴单独比）。 */
    public static boolean nearEnough(int dx, int dz) {
        return Math.abs(dx) <= MAX_DISTANCE && Math.abs(dz) <= MAX_DISTANCE;
    }
}
