package com.lingchat.maidbridge.voice;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.maid.MaidCondition;
import com.lingchat.maidbridge.maid.MaidQueries;
import java.util.Base64;
import net.minecraft.server.MinecraftServer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * {@code play_tts}：在指定女仆的位置播放一句 TTS 语音。
 *
 * <p>插件发来的是 WAV 的 base64；这里只做校验与解码，再把整段语音分片广播给女仆附近
 * 装了本模组的玩家，各客户端自己拼装与播放。单人游戏里房主也走同一条路。
 */
public class PlayTtsHandler implements MessageHandler {
    private static final Logger LOGGER = LoggerFactory.getLogger("lingchat_maid_bridge");

    private final MinecraftServer server;

    public PlayTtsHandler(MinecraftServer server) {
        this.server = server;
    }

    @Override
    public JsonObject handle(JsonObject request) {
        if (server == null) {
            return Envelope.error(MaidQueries.NOT_READY);
        }
        JsonObject data = Envelope.data(request);
        EntityMaid maid = MaidQueries.requested(server, request);
        if (maid == null) {
            return Envelope.error(MaidQueries.NOT_FOUND);
        }
        // 昏迷时游戏内保持安静；插件那边会把语音改回本机
        if (MaidCondition.isUnconscious(maid)) {
            return null;
        }
        String format = Envelope.string(data, "format");
        if (!format.isEmpty() && !"wav".equalsIgnoreCase(format)) {
            return Envelope.error("不支持的语音格式：" + format);
        }
        String encoded = Envelope.string(data, "audio");
        if (encoded.isEmpty()) {
            return Envelope.error("语音内容为空");
        }
        byte[] bytes;
        try {
            bytes = Base64.getDecoder().decode(encoded);
        } catch (IllegalArgumentException e) {
            return Envelope.error("语音不是合法的 base64");
        }
        if (bytes.length > WavPcm.MAX_BYTES) {
            return Envelope.error("语音文件过大（" + bytes.length + " 字节）");
        }
        if (WavPcm.parse(bytes) == null) {
            LOGGER.warn("收到不支持的 TTS 音频（只支持 16-bit PCM WAV），已跳过");
            return Envelope.error("不支持的音频格式（只支持 16-bit PCM WAV）");
        }
        TtsPayload.broadcast(maid, bytes);
        // 单向推送，不需要回执。
        return null;
    }
}
