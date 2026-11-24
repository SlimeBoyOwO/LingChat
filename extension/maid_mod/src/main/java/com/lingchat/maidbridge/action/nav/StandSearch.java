package com.lingchat.maidbridge.action.nav;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.SearchBudget;
import com.lingchat.maidbridge.debug.PathDebug;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Set;
import java.util.function.Predicate;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.pathfinder.Path;

/**
 * 找站位：目标周围哪一格站得住、又能对它动手。
 *
 * <p>判定条件由调用方给（放置是「够得着又看得见」，挖掘是「能够到、能挖」），这里只负责
 * 找候选、比路径长短。优先站到已经盖好的图纸格上，再按寻路步数挑最近的一个：只比直线
 * 距离会选中「看着近、其实要绕一大圈」的站位，也会让屋顶周围的高台排到前面。
 *
 * <p>每次寻路都从 budget 领一次额度：屋里屋外不通时，逐个候选算路能卡住整个服务端；
 * 额度用完就带着当前最好的候选回去，下一 tick 再接着算。
 */
public final class StandSearch {
    /** 第一圈找多宽：够得着是 4.5 格，只扫 ±2 会漏掉屋顶外沿那些站位。 */
    private static final int RING = 4;
    private static final int DOWN = 6;
    private static final int UP = 3;
    /** 第一圈一个都找不到时扩到这一圈，再远就没意义了：够不着也走不回来。 */
    private static final int RING_WIDE = 6;
    private static final int DOWN_WIDE = 8;
    private static final int UP_WIDE = 5;
    /** 最多验几个候选的寻路：每个候选都要算一次路径。 */
    private static final int PATH_CHECKS = 24;
    /** 找到几个能走到的就按路径步数挑最短的，不必把二十四个都算完。 */
    private static final int PATH_RANK = 8;
    /** 每圈最多判这么多格：扩圈之后候选数量翻几倍，判定次数得有个数。 */
    private static final int MAX_LOOKUPS = 1400;
    /** 视线检查比寻路便宜，但也别把一圈全扫完。 */
    private static final int SIGHT_SCAN = 64;
    /** 出屋找露天站位时最多验几个候选。 */
    private static final int STAGE_CHECKS = 8;

    private StandSearch() {
    }

    /**
     * 找一个能对 target 动手的站位。
     *
     * @param skip        这个目标试过、不行的站位
     * @param unreachable 从她上次站的位置算过、走不到的站位
     * @param canAct      站到这一格之后能不能动手
     */
    public static BlockPos forBuilding(
            EntityMaid maid, BlockPos target, Set<BlockPos> built,
            Set<BlockPos> skip, Set<BlockPos> unreachable, Predicate<BlockPos> canAct,
            SearchBudget budget) {
        if (!(maid.level() instanceof ServerLevel)) {
            return null;
        }
        PathDebug.searchRange(maid, target, RING, DOWN, UP);
        BlockPos best = pick(
                maid, target, buildingCandidates(maid, target, RING, DOWN, UP),
                built, skip, unreachable, canAct, budget);
        if (best != null || budget.spent()) {
            return best;
        }
        // 近的几圈一处都站不上：扩一圈再找。屋顶外沿能站的往往在三四格外
        PathDebug.searchRange(maid, target, RING_WIDE, DOWN_WIDE, UP_WIDE);
        return pick(
                maid, target,
                buildingCandidates(maid, target, RING_WIDE, DOWN_WIDE, UP_WIDE),
                built, skip, unreachable, canAct, budget);
    }

    private static BlockPos pick(
            EntityMaid maid, BlockPos target, List<BlockPos> candidates, Set<BlockPos> built,
            Set<BlockPos> skip, Set<BlockPos> unreachable, Predicate<BlockPos> canAct,
            SearchBudget budget) {
        int feetY = maid.blockPosition().getY();
        // 不站在头顶上方、离得近的先看；是不是站在已盖好的面上只当同距时的加分项
        candidates.sort(Comparator
                .comparingInt((BlockPos stand) -> stand.getY() <= feetY + 1 ? 0 : 1)
                .thenComparingDouble(stand -> stand.distSqr(target))
                .thenComparingInt(stand -> built.contains(stand.below()) ? 0 : 1));
        int evaluated = 0;
        int reachable = 0;
        int scanned = 0;
        BlockPos best = null;
        double bestCost = Double.MAX_VALUE;
        for (BlockPos stand : candidates) {
            if (evaluated >= PATH_CHECKS || scanned >= SIGHT_SCAN) {
                break;
            }
            if (skip.contains(stand) || unreachable.contains(stand)) {
                PathDebug.probe(maid, target, stand, PathDebug.Verdict.SKIPPED);
                continue;
            }
            scanned++;
            if (!canAct.test(stand)) {
                PathDebug.probe(maid, target, stand, PathDebug.Verdict.NO_SIGHT);
                continue;
            }
            evaluated++;
            if (stand.closerToCenterThan(maid.position(), 2.0)) {
                PathDebug.probe(maid, target, stand, PathDebug.Verdict.REACHABLE);
                return stand;
            }
            if (!budget.take()) {
                break;
            }
            Path path = maid.getNavigation().createPath(stand, 0);
            if (path == null || !path.canReach()) {
                unreachable.add(stand);
                PathDebug.probe(maid, target, stand, PathDebug.Verdict.NO_PATH);
                continue;
            }
            PathDebug.probe(maid, target, stand, PathDebug.Verdict.REACHABLE);
            reachable++;
            double cost = path.getNodeCount();
            if (cost < bestCost) {
                bestCost = cost;
                best = stand;
            }
            if (reachable >= PATH_RANK) {
                break;
            }
        }
        return best;
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

    /**
     * 盖房子的一圈候选：目标和这一圈里站得下的格子，外加她自己脚下那一格。
     *
     * <p>她此刻站的地方可能正好够得着，但落在这一圈之外，漏掉它会为同一件事多走一趟。
     */
    private static List<BlockPos> buildingCandidates(
            EntityMaid maid, BlockPos target, int radius, int down, int up) {
        List<BlockPos> candidates = candidates(maid, target, radius, down, up);
        BlockPos here = maid.blockPosition();
        if (!here.equals(target) && Terrain.standable(maid.level(), here)
                && !candidates.contains(here)) {
            candidates.add(here);
        }
        return candidates;
    }

    /**
     * 垫脚前先出屋：目标附近一处头顶露天的落脚点。
     *
     * <p>在屋里往房顶垫，头顶先撞到天花板，垫不上去；找一处能看见天空的站位让她先走到
     * 外面，再沿外墙往上垫。候选就在目标周围，太远的空地没有用。
     */
    public static BlockPos stageFor(
            EntityMaid maid, BlockPos target, SearchBudget budget) {
        if (!(maid.level() instanceof ServerLevel level)) {
            return null;
        }
        List<BlockPos> candidates = candidates(maid, target, 2, 6, 2);
        int checks = 0;
        for (BlockPos stand : candidates) {
            if (checks >= STAGE_CHECKS) {
                break;
            }
            if (!level.isLoaded(stand.above()) || !level.canSeeSky(stand.above())) {
                continue;
            }
            // 她现在站的这一处不算：要找的是「走出去」的地方
            if (stand.closerToCenterThan(maid.position(), 2.0)) {
                continue;
            }
            checks++;
            if (!budget.take()) {
                return null;
            }
            if (maid.canPathReach(stand)) {
                return stand;
            }
        }
        return null;
    }
}
