package com.lingchat.maidbridge.voice;

import java.lang.reflect.Method;
import net.neoforged.fml.loading.FMLEnvironment;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * 公共侧与客户端播放入口之间的隔离层。
 *
 * <p>专用服务器的 classpath 里没有 {@code Minecraft}，公共类直接引用客户端类可能在校验阶段
 * 就把整条链拉起来。这里只在确认是客户端之后用反射加载 {@link ClientVoice}（与 CarryOn 的
 * 隔离方式一致），服务端永远不碰它。
 */
public final class VoiceSide {
    private static final Logger LOGGER = LoggerFactory.getLogger("lingchat_maid_bridge");
    private static Method playMethod;
    private static boolean resolved;

    private VoiceSide() {
    }

    /** 在客户端线程上播放；专用服务器或客户端音频不可用时静默跳过。 */
    public static void play(int entityId, WavPcm wav) {
        if (!FMLEnvironment.dist.isClient()) {
            return;
        }
        Method method = playMethod();
        if (method == null) {
            return;
        }
        try {
            method.invoke(null, entityId, wav);
        } catch (Throwable e) {
            LOGGER.warn("游戏内语音播放失败：{}", e.toString());
        }
    }

    private static synchronized Method playMethod() {
        if (!resolved) {
            resolved = true;
            try {
                Class<?> type = Class.forName("com.lingchat.maidbridge.voice.ClientVoice");
                playMethod = type.getMethod("play", int.class, WavPcm.class);
            } catch (Throwable e) {
                LOGGER.warn("客户端语音播放不可用：{}", e.toString());
            }
        }
        return playMethod;
    }
}
