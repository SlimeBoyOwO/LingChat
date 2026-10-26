package com.lingchat.maidbridge.action.place;

import java.util.Locale;

/** 放置的方向口径：协议里的 facing 写法 → 原版 yaw。纯逻辑，好单测。 */
public final class PlaceRules {
    private PlaceRules() {
    }

    /**
     * 协议里的 facing → 原版 yaw；认不出给 null（按她自己的朝向放）。
     *
     * <p>原版 yaw 的四个方向：南 0、西 90、北 180、东 270。中文说法由插件先翻成这几个词。
     */
    public static Float yawOf(String facing) {
        if (facing == null) {
            return null;
        }
        return switch (facing.trim().toLowerCase(Locale.ROOT)) {
            case "south" -> 0.0F;
            case "west" -> 90.0F;
            case "north" -> 180.0F;
            case "east" -> 270.0F;
            default -> null;
        };
    }
}
