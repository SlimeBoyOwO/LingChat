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
        String maidId = MonitoredMaidHandler.get();
        if (player == null || maidId.isEmpty() || player.getServer() == null) {
            return null;
        }
        EntityMaid maid = MaidQueries.findMaidById(player.getServer(), maidId);
        if (maid == null || maid.getOwner() == null) {
            return null;
        }
        return maid.getOwner().getUUID().equals(player.getUUID()) ? maid : null;
    }
}
