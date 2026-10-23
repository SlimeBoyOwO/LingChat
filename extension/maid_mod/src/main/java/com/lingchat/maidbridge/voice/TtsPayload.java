package com.lingchat.maidbridge.voice;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.LingChatMaidBridge;
import java.util.Arrays;
import java.util.concurrent.atomic.AtomicInteger;
import net.minecraft.network.ConnectionProtocol;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.neoforged.neoforge.network.PacketDistributor;
import net.neoforged.neoforge.network.registration.NetworkRegistry;
import net.neoforged.neoforge.network.registration.PayloadRegistrar;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * 语音分片包：一句 WAV 按 {@link #CHUNK_BYTES} 切成若干片发给女仆附近的玩家。
 *
 * <p>单个自定义包的载荷上限是 1 MiB，而语音最长能到 8 MiB，所以必须分片。客户端由
 * {@link TtsAssembler} 按传输 id 拼回整段再播放。通道声明为可选：没装模组的玩家既不会
 * 因为缺通道连不上服务器，也不会真的收到这些包。
 */
public record TtsPayload(
        int entityId, int transferId, int index, int total, int totalBytes, byte[] data)
        implements CustomPacketPayload {
    private static final Logger LOGGER = LoggerFactory.getLogger(LingChatMaidBridge.MOD_ID);

    /** 分片大小：远低于 1 MiB 的包上限，一句 1.5 MB 的语音也只要 3 个包。 */
    public static final int CHUNK_BYTES = 512 * 1024;

    /** 与声音实例约 64 格的可听范围对齐。 */
    private static final double HEAR_RADIUS = 64;

    public static final int MAX_CHUNKS = chunkCount(WavPcm.MAX_BYTES);

    public static final CustomPacketPayload.Type<TtsPayload> TYPE = new CustomPacketPayload.Type<>(
            ResourceLocation.fromNamespaceAndPath(LingChatMaidBridge.MOD_ID, "tts"));

    public static final StreamCodec<RegistryFriendlyByteBuf, TtsPayload> STREAM_CODEC = StreamCodec.of(
            (buf, payload) -> {
                buf.writeVarInt(payload.entityId);
                buf.writeVarInt(payload.transferId);
                buf.writeVarInt(payload.index);
                buf.writeVarInt(payload.total);
                buf.writeVarInt(payload.totalBytes);
                buf.writeByteArray(payload.data);
            },
            buf -> new TtsPayload(
                    buf.readVarInt(),
                    buf.readVarInt(),
                    buf.readVarInt(),
                    buf.readVarInt(),
                    buf.readVarInt(),
                    buf.readByteArray(CHUNK_BYTES)));

    private static final AtomicInteger NEXT_TRANSFER = new AtomicInteger();

    @Override
    public CustomPacketPayload.Type<? extends CustomPacketPayload> type() {
        return TYPE;
    }

    public static int chunkCount(int bytes) {
        return (bytes + CHUNK_BYTES - 1) / CHUNK_BYTES;
    }

    public static void register(PayloadRegistrar registrar) {
        // 声明为可选：没装模组的玩家不必因此连不上服务器，发送前也会按通道过滤。
        registrar.optional().playToClient(TYPE, STREAM_CODEC, (payload, context) -> {
            WavPcm wav = TtsAssembler.accept(payload);
            if (wav != null) {
                LOGGER.info("收到游戏内语音：{} 字节", wav.length());
                VoiceSide.play(payload.entityId(), wav);
            }
        });
    }

    /** 把一段完整的 WAV 广播给女仆周围能听到的玩家（单人游戏里也包括房主自己）。 */
    public static void broadcast(EntityMaid maid, byte[] audio) {
        if (!(maid.level() instanceof ServerLevel level)) {
            return;
        }
        int total = chunkCount(audio.length);
        int transferId = NEXT_TRANSFER.getAndIncrement();
        double reach = HEAR_RADIUS * HEAR_RADIUS;
        int nearby = 0;
        int receivers = 0;
        for (ServerPlayer player : level.players()) {
            if (player.distanceToSqr(maid) > reach) {
                continue;
            }
            nearby++;
            // 可选通道：没协商到这条通道的玩家收不到，也不能发，发了会直接抛异常。
            if (!NetworkRegistry.hasChannel(
                    player.connection.getConnection(), ConnectionProtocol.PLAY, TYPE.id())) {
                continue;
            }
            receivers++;
            for (int index = 0; index < total; index++) {
                int from = index * CHUNK_BYTES;
                int to = Math.min(audio.length, from + CHUNK_BYTES);
                PacketDistributor.sendToPlayer(
                        player,
                        new TtsPayload(
                                maid.getId(),
                                transferId,
                                index,
                                total,
                                audio.length,
                                Arrays.copyOfRange(audio, from, to)));
            }
        }
        LOGGER.info(
                "游戏内语音 {} 字节 / {} 片：附近 {} 名玩家，{} 名可接收",
                audio.length,
                total,
                nearby,
                receivers);
    }
}
