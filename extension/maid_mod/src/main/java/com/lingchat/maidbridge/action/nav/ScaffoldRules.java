package com.lingchat.maidbridge.action.nav;

/**
 * 垫脚与搭路的额度：一次施工最多动多少地形。
 *
 * <p>垫脚只是兜底，不能把整座楼都拿去换台阶；额度按目标各算一份，由
 * {@link RouteSearch} 在读边时扣减。
 */
public final class ScaffoldRules {
    /** 一次施工最多同时立着这么多块：垫完走过去就拆，账上是同时在场的数量。 */
    public static final int MAX_BLOCKS = 16;
    /** 同一个目标最多为它垫几次脚（按升高次数算，不是放下的方块数）。高一格算一次。 */
    public static final int MAX_STEPS = 12;
    /** 同一个目标最多为它平铺几块路：平路一格一块，走不过去的缺口总有走完的时候。 */
    public static final int MAX_BRIDGE = 8;

    private ScaffoldRules() {
    }
}
