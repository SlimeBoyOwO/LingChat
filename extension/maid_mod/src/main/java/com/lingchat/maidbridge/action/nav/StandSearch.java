package com.lingchat.maidbridge.action.nav;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;

/**
 * 目标周围站得住的格子，由近及远排好。
 *
 * <p>挖掘用它挑能动手的候选；施工的站位改由 {@link RouteSearch} 在落脚点图上找，
 * 不再先让原版寻路把候选筛一遍。
 */
public final class StandSearch {
    /** 每圈最多判这么多格：扩圈之后候选数量翻几倍，判定次数得有个数。 */
    private static final int MAX_LOOKUPS = 1400;

    private StandSearch() {
    }

    /** 目标周围一圈站得下的格子，由近及远排好。 */
    public static List<BlockPos> candidates(
            EntityMaid maid, BlockPos target, int radius, int down, int up) {
        ServerLevel level = (ServerLevel) maid.level();
        List<BlockPos> candidates = new ArrayList<>();
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        int looked = 0;
        scan:
        for (int ring = 0; ring <= radius; ring++) {
            for (int dx = -ring; dx <= ring; dx++) {
                for (int dz = -ring; dz <= ring; dz++) {
                    if (Math.max(Math.abs(dx), Math.abs(dz)) != ring) {
                        continue;
                    }
                    for (int dy = -down; dy <= up; dy++) {
                        if (dx == 0 && dy == 0 && dz == 0) {
                            continue;
                        }
                        if (++looked > MAX_LOOKUPS) {
                            break scan;
                        }
                        pos.setWithOffset(target, dx, dy, dz);
                        BlockPos stand = pos.immutable();
                        if (!Terrain.standable(level, stand)) {
                            continue;
                        }
                        if (maid.hasRestriction() && !maid.isWithinRestriction(stand)) {
                            continue;
                        }
                        candidates.add(stand);
                    }
                }
            }
        }
        candidates.sort(Comparator.comparingDouble(stand -> stand.distSqr(target)));
        return candidates;
    }
}
