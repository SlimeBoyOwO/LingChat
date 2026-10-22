package com.lingchat.maidbridge.perception.event;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.MaidBridgeConfig;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.integration.carry.CarryOnBridge;
import com.lingchat.maidbridge.integration.carry.CarryOnPresent;
import com.lingchat.maidbridge.maid.MaidCondition;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.maid.MonitoredMaidHandler;
import com.lingchat.maidbridge.perception.EventFrames;
import java.util.Optional;
import net.minecraft.advancements.AdvancementHolder;
import net.minecraft.advancements.DisplayInfo;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.neoforged.neoforge.event.entity.player.AdvancementEvent;

/**
 * 主人拿到了成就（原版「进度」）。
 *
 * <p>这是世界事件，不是她身边的变化，所以跟着 {@code perceptionEnabled} 走，不跟
 * {@code noticeEnabled}。只在「拿到成就的正是她主人」时上报。
 *
 * <p>没展示信息的进度（配方解锁这类技术性条目）一律不报；中文名由插件按 id 定，模组只给
 * 注册表 id、游戏内的标题与 `task` / `goal` / `challenge` 三档类型。
 */
public final class AchievementReporter {
    private AchievementReporter() {
    }

    public static void onAdvancement(AdvancementEvent.AdvancementEarnEvent event) {
        if (!MaidBridgeConfig.PERCEPTION_ENABLED.get()) {
            return;
        }
        if (!(event.getEntity() instanceof ServerPlayer player)) {
            return;
        }
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge == null || !bridge.hasClients()) {
            return;
        }
        MinecraftServer server = player.getServer();
        if (server == null) {
            return;
        }
        String maidId = MonitoredMaidHandler.get();
        if (maidId.isEmpty() || !belongsTo(server, maidId, player)) {
            return;
        }
        // 昏迷时她不知道主人拿到了什么；醒来后状态会重新交代
        EntityMaid maid = MaidQueries.findMaidById(server, maidId);
        if ((maid != null && MaidCondition.isUnconscious(maid))
                || MaidCondition.isUnconscious(maidId)) {
            return;
        }

        AdvancementHolder advancement = event.getAdvancement();
        Optional<DisplayInfo> display = advancement.value().display();
        if (display.isEmpty()) {
            return;
        }
        ResourceLocation id = advancement.id();
        if (id.getPath().startsWith("recipes/")) {
            return;
        }

        JsonObject frame = EventFrames.base("player_advancement", maidId);
        frame.addProperty("advancement", id.toString());
        frame.addProperty("title", display.get().getTitle().getString());
        frame.addProperty("frame", display.get().getType().getSerializedName());
        bridge.broadcastEvent(frame);
    }

    /**
     * 拿到成就的这个玩家是不是她主人。
     *
     * <p>她走远到区块卸载、或被 Carry On 抱在怀里时实体查不到；主人 uuid 在绑定时与每次
     * 见到她时都记着（{@link MonitoredMaidHandler#ownerId()}），所以这两段空档也能认人。
     * 还没记下主人（一次都没见过）时才现查，最后再退回「抱着她的人」。
     */
    private static boolean belongsTo(MinecraftServer server, String maidId, ServerPlayer player) {
        String ownerId = MonitoredMaidHandler.ownerId();
        if (!ownerId.isEmpty()) {
            return ownerId.equals(player.getStringUUID());
        }
        EntityMaid maid = MaidQueries.findMaidById(server, maidId);
        if (maid != null) {
            return player.getUUID().equals(maid.getOwnerUUID());
        }
        return CarryOnPresent.LOADED && CarryOnBridge.carrier(server, maidId) == player;
    }
}
