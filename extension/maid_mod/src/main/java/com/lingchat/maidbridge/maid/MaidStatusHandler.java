package com.lingchat.maidbridge.maid;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.bridge.Protocol;
import net.minecraft.server.MinecraftServer;

/** {@code get_maid_status}：把当前加载着的女仆都报一遍。 */
public class MaidStatusHandler implements MessageHandler {
    private final MinecraftServer server;

    public MaidStatusHandler(MinecraftServer server) {
        this.server = server;
    }

    @Override
    public JsonObject handle(JsonObject request) {
        if (server == null) {
            return Envelope.error(MaidQueries.NOT_READY);
        }
        JsonArray maids = new JsonArray();
        for (EntityMaid maid : MaidQueries.getAllMaids(server)) {
            maids.add(MaidQueries.serialize(maid));
        }
        JsonObject data = new JsonObject();
        data.add("maids", maids);
        return Envelope.response(Protocol.MAID_STATUS, data);
    }
}
