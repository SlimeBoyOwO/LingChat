package com.lingchat.maidbridge.perception.event;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.maid.FavoriteFood;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.maid.MonitoredMaidHandler;
import com.lingchat.maidbridge.perception.EventFrames;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;

/**
 * 主人拿着她最喜欢吃的东西时，她凑过去讨的那一下。
 *
 * <p>只认她真的讨起来了（车万女仆的讨食任务已经把姿态翻过去）这一种情况：主人在
 * 六格外的山坡上举着苹果，她看不见也过不去，这时候说「她眼巴巴地盯着你手里的
 * 苹果」是假的。拿在主手、走到跟前这些条件全交给原版任务判，这里只看结果。
 *
 * <p>收起来再拿出来会各报一次，所以两次上报之间留一段最短间隔，换快捷栏时不刷屏。
 */
public final class FavoriteFoodReporter {
    /** 两次上报之间的最短间隔（tick）。 */
    private static final int PUSH_GAP_TICKS = 200;

    private static String maidId = "";
    private static boolean begging;
    private static int gap;

    private FavoriteFoodReporter() {
    }

    /** 每个服务端 tick 调一次（主线程）。 */
    public static void tick(MinecraftServer server) {
        if (gap > 0) {
            gap--;
        }
        BridgeServer bridge = LingChatMaidBridge.server();
        EntityMaid maid = bridge != null && bridge.hasClients()
                ? MaidQueries.findMaidById(server, MonitoredMaidHandler.get())
                : null;
        String nowId = maid == null ? "" : maid.getStringUUID();
        boolean now = begging(maid);
        boolean entered = now && (!begging || !maidId.equals(nowId));
        maidId = nowId;
        begging = now;
        if (!entered || gap > 0) {
            return;
        }
        ServerPlayer owner = MaidQueries.owner(maid);
        JsonObject frame = EventFrames.base("favorite_food", maidId);
        frame.addProperty("item", FavoriteFood.id(owner.getMainHandItem()));
        bridge.broadcastEvent(frame);
        gap = PUSH_GAP_TICKS;
    }

    /** 停服、换存档时清干净。 */
    public static void clear() {
        maidId = "";
        begging = false;
        gap = 0;
    }

    /** 她这会儿正为主人手里那件东西讨食。 */
    private static boolean begging(EntityMaid maid) {
        if (maid == null || !maid.isAlive() || !maid.isBegging()) {
            return false;
        }
        ServerPlayer owner = MaidQueries.owner(maid);
        return owner != null && FavoriteFood.holding(owner.getMainHandItem());
    }
}
