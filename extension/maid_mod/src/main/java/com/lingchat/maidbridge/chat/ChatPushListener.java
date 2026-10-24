package com.lingchat.maidbridge.chat;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.maid.MonitoredMaidHandler;
import net.minecraft.server.level.ServerPlayer;
import net.neoforged.neoforge.event.ServerChatEvent;

/**
 * 玩家在游戏里打字 → 推给 LingChat。
 *
 * <p>只在玩家真的发言时触发。女仆自己的气泡走的是另一条路，不会被这里再收一遍，
 * 所以不会出现自己跟自己说话。
 */
public final class ChatPushListener {
    private ChatPushListener() {
    }

    public static void onServerChat(ServerChatEvent event) {
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge == null || !bridge.hasClients()) {
            return;
        }
        ServerPlayer player = event.getPlayer();
        EntityMaid maid = monitoredOwnerMaid(player);
        if (maid == null) {
            return;
        }

        JsonObject data = new JsonObject();
        data.addProperty("event_type", "chat");
        data.addProperty("sender", player.getName().getString());
        data.addProperty("message", event.getRawText());
        data.addProperty("x", player.getX());
        data.addProperty("y", player.getY());
        data.addProperty("z", player.getZ());
        data.addProperty("maid_id", maid.getStringUUID());
        bridge.broadcastChatMessage(data);
    }

    /** 监控中的女仆，且它的主人必须正好是说话的这个玩家。 */
    private static EntityMaid monitoredOwnerMaid(ServerPlayer player) {
        if (player == null || player.getServer() == null) {
            return null;
        }
        String maidId = MonitoredMaidHandler.get();
        EntityMaid maid = maidId.isEmpty() ? null : MaidQueries.findMaidById(player.getServer(), maidId);
        if (maid == null) {
            // 插件连上的那一刻她可能还没被召唤出来，绑定就没发出去；之后聊天帧缺绑定
            // 根本到不了插件，也就没有下一次补发。另一种是死后用胶片复活：胶片不存 uuid，
            // 复活出来是新的一只，旧绑定永远查不到。两种情况都在第一句发言里改认她的女仆。
            maid = ownerMaid(player);
            if (maid == null) {
                return null;
            }
            MonitoredMaidHandler.adopt(maid);
        }
        if (maid == null || maid.getOwner() == null) {
            return null;
        }
        return maid.getOwner().getUUID().equals(player.getUUID()) ? maid : null;
    }

    /** 说话玩家自己的女仆里最近的那一只；优先同一维度。 */
    private static EntityMaid ownerMaid(ServerPlayer player) {
        EntityMaid nearest = null;
        EntityMaid elsewhere = null;
        double best = Double.MAX_VALUE;
        for (EntityMaid maid : MaidQueries.getAllMaids(player.getServer())) {
            if (!maid.isAlive() || maid.getOwner() == null
                    || !maid.getOwner().getUUID().equals(player.getUUID())) {
                continue;
            }
            if (maid.level() != player.level()) {
                elsewhere = elsewhere == null ? maid : elsewhere;
                continue;
            }
            double distance = maid.distanceToSqr(player);
            if (distance < best) {
                best = distance;
                nearest = maid;
            }
        }
        return nearest != null ? nearest : elsewhere;
    }
}
