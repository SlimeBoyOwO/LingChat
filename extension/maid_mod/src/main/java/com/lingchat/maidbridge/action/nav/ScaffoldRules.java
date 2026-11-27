package com.lingchat.maidbridge.action.nav;

/**
 * 搭路的额度：一次施工最多平铺多少块路。
 *
 * <p>垫脚与升高不再设上限，只看她身上有没有方块；平铺是主动改地形，仍按目标各算
 * 一份，由 {@link RouteSearch} 在读边时扣减。
 */
public final class ScaffoldRules {
    /** 同一个目标最多为它平铺几块路：平路一格一块，走不过去的缺口总有走完的时候。 */
    public static final int MAX_BRIDGE = 8;

    private ScaffoldRules() {
    }
}
