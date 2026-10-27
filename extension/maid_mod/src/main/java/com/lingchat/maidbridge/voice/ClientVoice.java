package com.lingchat.maidbridge.voice;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import net.minecraft.client.Minecraft;
import net.minecraft.client.sounds.SoundManager;
import net.minecraft.world.entity.Entity;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * 客户端侧的语音播放入口。语音分片收齐后由 {@link TtsAssembler} 调用这里。
 *
 * <p>这个类引用客户端专有类型，公共侧只能在 {@code FMLEnvironment.dist.isClient()}
 * 成立之后再碰它，专用服务器不会加载（与 CarryOnPresent 的隔离方式一致）。
 */
@OnlyIn(Dist.CLIENT)
public final class ClientVoice {
    private static final Logger LOGGER = LoggerFactory.getLogger("lingchat_maid_bridge");

    private static TtsSoundInstance current;

    private ClientVoice() {
    }

    /** 在客户端线程上替掉上一句，播放这一句。 */
    public static void play(int entityId, WavPcm wav) {
        Minecraft minecraft = Minecraft.getInstance();
        minecraft.execute(() -> {
            if (minecraft.level == null) {
                return;
            }
            Entity entity = minecraft.level.getEntity(entityId);
            if (!(entity instanceof EntityMaid maid) || !maid.isAlive()) {
                LOGGER.warn("游戏内语音找不到女仆实体 {}，可能还没同步到本客户端", entityId);
                return;
            }
            SoundManager manager = minecraft.getSoundManager();
            if (current != null) {
                manager.stop(current);
            }
            current = new TtsSoundInstance(maid, wav);
            manager.play(current);
        });
    }
}
