package com.lingchat.maidbridge.perception.world;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.nearby.SurroundingsDiff;
import com.lingchat.maidbridge.perception.nearby.SurroundingsReader;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.level.Level;

/**
 * 读一份「世界」的快照：在哪、什么地形、什么天气、什么时辰、双方血量。
 *
 * <p>模组里「碰游戏」的三处分工：改状态在 {@link MaidActuator}，读全局这一层在这里，
 * 读她身边那一小片在 {@link SurroundingsReader}。判定一律不碰游戏
 * （{@link PerceptionDiff} 与 {@link SurroundingsDiff}），于是都能脱开 Minecraft 跑单测。
 */
public final class PerceptionReader {
    private PerceptionReader() {
    }

    public static PerceptionSnapshot read(EntityMaid maid) {
        // 地形取玩家所在的位置：女仆蹲在家里时，拿它的坐标报「周围是什么地形」是错的
        LivingEntity owner = maid.getOwner();
        LivingEntity focus = owner instanceof ServerPlayer ? owner : maid;
        Level level = focus.level();
        boolean hasSky = level.dimensionType().hasSkyLight();
        String biome = level.getBiome(focus.blockPosition())
                .unwrapKey()
                .map(key -> key.location().toString())
                .orElse("");

        String playerName = "";
        double playerHealth = 0;
        double playerMaxHealth = 0;
        if (owner instanceof ServerPlayer player) {
            playerName = player.getName().getString();
            playerHealth = player.getHealth();
            playerMaxHealth = player.getMaxHealth();
        }

        return new PerceptionSnapshot(
                maid.getStringUUID(),
                level.dimension().location().toString(),
                biome,
                hasSky,
                hasSky && level.isRaining(),
                hasSky && level.isThundering(),
                level.getDayTime(),
                playerName,
                playerHealth,
                playerMaxHealth,
                maid.getName().getString(),
                maid.getHealth(),
                maid.getMaxHealth(),
                MaidQueries.posture(maid));
    }
}
