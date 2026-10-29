package com.lingchat.maidbridge.perception.spatial;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import java.util.HashMap;
import java.util.Map;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.state.BlockState;

/**
 * 她记得的方块：看见过、还没忘。
 *
 * <p>只在主线程用。插件每次调用都是新的解释器，跨调用记不住东西；「刚才那边」只能由
 * 常驻的模组来说。按方块种类记最近看见的那一格，超过 TTL 就当没看见——她记的是
 * 「刚才附近有铁矿石」，不是一张永久的扫描件。
 *
 * <p>动作也会写这里：挖开一格，贴着的六格现在看得见了。
 */
public final class TerrainMemory {
    /** 一眼算多久：半分钟。再久就不该说「刚才」了。 */
    public static final long TTL_TICKS = 600;
    /** 每种女仆最多记多少种方块：只留最近看见的那些，免得跑一趟矿洞把内存撑大。 */
    private static final int MAX_IDS = 64;

    private static final Map<UUID, Map<String, Remembered>> BY_MAID = new HashMap<>();

    /** 某种方块最近一次被看见的位置与时刻。 */
    public record Remembered(BlockPos pos, long tick) {
    }

    private TerrainMemory() {
    }

    /** 这次看见了这一种方块，记下最近的那一格与时刻。 */
    public static void remember(EntityMaid maid, String blockId, BlockPos pos, long tick) {
        Map<String, Remembered> seen = BY_MAID.computeIfAbsent(maid.getUUID(), id -> new HashMap<>());
        if (seen.size() >= MAX_IDS && !seen.containsKey(blockId)) {
            evictOldest(seen);
        }
        seen.put(blockId, new Remembered(pos.immutable(), tick));
    }

    /** 她记得这一种方块吗；过了 TTL 顺手清掉。 */
    public static Remembered recall(EntityMaid maid, String blockId, long tick) {
        Map<String, Remembered> seen = BY_MAID.get(maid.getUUID());
        if (seen == null) {
            return null;
        }
        Remembered found = seen.get(blockId);
        if (found == null) {
            return null;
        }
        if (tick - found.tick() > TTL_TICKS) {
            seen.remove(blockId);
            return null;
        }
        return found;
    }

    /** 刚挖开一格：贴着的六格现在看得见了，记下来。 */
    public static void reveal(EntityMaid maid, BlockPos mined, long tick) {
        Level level = maid.level();
        for (Direction direction : Direction.values()) {
            BlockPos neighbour = mined.relative(direction);
            if (!level.isLoaded(neighbour)) {
                continue;
            }
            BlockState state = level.getBlockState(neighbour);
            if (state.isAir()) {
                continue;
            }
            remember(maid, idOf(state), neighbour, tick);
        }
    }

    /** 停服或换存档时清干净：别让上一个世界的记忆跟着她。 */
    public static void clear() {
        BY_MAID.clear();
    }

    private static void evictOldest(Map<String, Remembered> seen) {
        String oldest = null;
        long oldestTick = Long.MAX_VALUE;
        for (Map.Entry<String, Remembered> entry : seen.entrySet()) {
            if (entry.getValue().tick() < oldestTick) {
                oldestTick = entry.getValue().tick();
                oldest = entry.getKey();
            }
        }
        if (oldest != null) {
            seen.remove(oldest);
        }
    }

    private static String idOf(BlockState state) {
        return BuiltInRegistries.BLOCK.getKey(state.getBlock()).toString();
    }
}
