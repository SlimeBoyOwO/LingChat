package com.lingchat.maidbridge.integration.carry;

import java.util.UUID;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import tschipp.carryon.common.carry.CarryOnData;
import tschipp.carryon.common.carry.CarryOnDataManager;

/**
 * 与 Carry On 的窄接口：她此刻是不是被某个人抱在怀里。
 *
 * <p>Carry On 抱起重物时会把实体**从世界里摘掉**（`RemovalReason.UNLOADED_WITH_PLAYER`），
 * 把它的完整 NBT 存在玩家身上的 carry 数据里；放下来时再按同一份 NBT 重建（UUID 保留）。
 * 所以被抱着的这段时间 {@code findMaidById} 查不到她，只能反过来问「谁手里的那份 NBT
 * 是这只女仆」。每个玩家一份数据，挨个问一遍即可。
 *
 * <p>本类只在 {@link CarryOnPresent#LOADED} 为真时才会被加载：Carry On 是可选依赖，
 * 没装时编译期引用的类在运行期并不存在。
 */
public final class CarryOnBridge {
    private CarryOnBridge() {
    }

    /** 玩家手里此刻抱着东西（实体或方块）。装没装 Carry On 由调用方先判。 */
    public static boolean isCarrying(net.minecraft.world.entity.player.Player player) {
        if (player == null) {
            return false;
        }
        CarryOnData carry = CarryOnDataManager.getCarryData(player);
        return carry != null && carry.isCarrying();
    }

    /** 抱着这只女仆的玩家；没被谁抱着时给 null。 */
    public static ServerPlayer carrier(MinecraftServer server, String maidId) {
        if (server == null || maidId == null || maidId.isEmpty()) {
            return null;
        }
        for (ServerPlayer player : server.getPlayerList().getPlayers()) {
            CarryOnData carry = CarryOnDataManager.getCarryData(player);
            if (carry == null || !carry.isCarrying(CarryOnData.CarryType.ENTITY)) {
                continue;
            }
            CompoundTag entity = carry.getContentNbt();
            if (entity == null || !entity.hasUUID("UUID")) {
                continue;
            }
            // 只认 UUID。实体类型 id 在 Carry On 存的那份 NBT 里理论上也有，但不同版本
            // 存法略有差别；UUID 是唯一的，拿它认人已经足够，多一道 id 检查只会多一个断点。
            UUID carried = entity.getUUID("UUID");
            if (carried.toString().equals(maidId)) {
                return player;
            }
        }
        return null;
    }
}
