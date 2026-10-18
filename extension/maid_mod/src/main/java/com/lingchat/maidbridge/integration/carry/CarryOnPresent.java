package com.lingchat.maidbridge.integration.carry;

import net.neoforged.fml.ModList;

/**
 * 「装没装 Carry On」这一个判断单独放一处。
 *
 * <p>本类**不许出现任何 {@code tschipp.carryon} 的类型**：没装那个模组时它的类根本不存在，
 * 一旦本类在加载时牵连到它们，整个桥接会跟着起不来。真正的调用都在 {@link CarryOnBridge}，
 * 只有这里为真时才会碰它。
 */
public final class CarryOnPresent {
    public static final String MOD_ID = "carryon";
    public static final boolean LOADED = ModList.get().isLoaded(MOD_ID);

    private CarryOnPresent() {
    }
}
