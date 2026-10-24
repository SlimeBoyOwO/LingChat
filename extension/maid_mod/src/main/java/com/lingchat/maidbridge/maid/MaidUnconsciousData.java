package com.lingchat.maidbridge.maid;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;

/**
 * 女仆身上的昏迷同步标记。
 *
 * <p>持久化 NBT 只在服务端读得到，客户端要靠这个同步布尔值决定躺姿碰撞箱。
 * 由 {@code EntityMaidDataMixin} 实现；没有实体时调用方拿到 false。
 */
public interface MaidUnconsciousData {
    boolean lingchat$isUnconscious();

    void lingchat$setUnconscious(boolean value);

    static boolean is(EntityMaid maid) {
        return maid instanceof MaidUnconsciousData data && data.lingchat$isUnconscious();
    }

    static void set(EntityMaid maid, boolean value) {
        if (maid instanceof MaidUnconsciousData data) {
            data.lingchat$setUnconscious(value);
        }
    }
}
