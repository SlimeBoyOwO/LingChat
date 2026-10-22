package com.lingchat.maidbridge.maid;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.bridge.Protocol;
import net.minecraft.server.MinecraftServer;

/**
 * {@code maid_state}：她现在是不是死的、是不是昏迷着。
 *
 * <p>死亡是持久状态，而且死后实体就不在世上，感知与聊天都推不出「她已经倒下」。
 * 插件每轮靠这一条把死亡状态重新写进台词表，模型才不会再把她当成站在旁边的人。
 * 昏迷同样需要每轮重申：她还有实体，所有指令都还在，只是叫不动。
 *
 * <p>{@code known} 表示这一局里模组真的见过她的死活：刚重启时它可能不知道上次留下的
 * 状态，插件据此退回台词表里最后一条状态旁白。
 */
public class MaidStateHandler implements MessageHandler {
    private final MinecraftServer server;

    public MaidStateHandler(MinecraftServer server) {
        this.server = server;
    }

    @Override
    public JsonObject handle(JsonObject request) {
        String maidId = MonitoredMaidHandler.get();
        EntityMaid maid = MaidQueries.findMaidById(server, maidId);
        if (maid != null) {
            // 见到活着的她也算「这一局知道她的状态」
            MonitoredMaidHandler.observeOwner(maid);
        }
        boolean unconscious = maid != null
                ? MaidCondition.isUnconscious(maid)
                : MaidCondition.isUnconscious(maidId);
        boolean onBed = unconscious && maid != null && MaidCondition.isOnBed(maid);
        int secondsLeft = maid != null ? MaidCondition.recoverySecondsLeft(maid) : 0;

        JsonObject data = new JsonObject();
        data.addProperty("maid_id", maidId);
        data.addProperty("dead", MonitoredMaidHandler.isDead());
        data.addProperty("known", MonitoredMaidHandler.isStateKnown() || unconscious);
        data.addProperty("unconscious", unconscious);
        data.addProperty("on_bed", onBed);
        data.addProperty("recovery_seconds", secondsLeft);
        return Envelope.response(Protocol.MAID_STATE_RESULT, data);
    }
}
