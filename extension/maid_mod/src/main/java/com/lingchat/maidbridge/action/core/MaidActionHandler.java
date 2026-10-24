package com.lingchat.maidbridge.action.core;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.chat.SendChatHandler;
import com.lingchat.maidbridge.maid.MaidQueries;
import net.minecraft.server.MinecraftServer;

/**
 * {@code maid_action}：让女仆做一件事（走到玩家/床边、坐下、站起、躺床、看向玩家）。
 *
 * <p>照 {@code SendChatHandler} 的骨架，自己不存状态：动作的推进全在
 * {@link MaidActionController} 上。这里只把请求落到「哪只女仆 + 哪个动作」，
 * 受理不了就当场给出中文原因。
 */
public class MaidActionHandler implements MessageHandler {
    private final MinecraftServer server;

    public MaidActionHandler(MinecraftServer server) {
        this.server = server;
    }

    @Override
    public JsonObject handle(JsonObject request) {
        if (server == null) {
            return Envelope.error(MaidQueries.NOT_READY);
        }
        MaidAction.ParseResult parsed = MaidAction.parse(Envelope.data(request));
        if (!parsed.accepted()) {
            return ActionFrames.rejected(null, parsed.errorCode(), parsed.error());
        }
        EntityMaid maid = MaidQueries.findMaidById(server, parsed.maidId());
        if (maid == null) {
            // 女仆跑远到区块卸载了也会走到这里
            return ActionFrames.rejected(parsed.action(), "maid_not_found", MaidQueries.NOT_FOUND);
        }
        return MaidActionController.start(server, maid, parsed.action());
    }
}
