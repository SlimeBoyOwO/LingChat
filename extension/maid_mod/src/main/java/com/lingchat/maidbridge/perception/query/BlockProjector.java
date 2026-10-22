package com.lingchat.maidbridge.perception.query;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.perception.nearby.Surroundings;
import com.lingchat.maidbridge.perception.spatial.BlockSight;
import com.lingchat.maidbridge.perception.spatial.LocalTerrain;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.Vec3;

/**
 * 把方块清单翻成模型能直接用的关系：方位、层高、挡路、局部步数与相连团。
 *
 * <p>只从 {@link LocalTerrain} 快照读，不再扫世界。步数只在八格内做一次有界估算，
 * 不是寻路；更远或算不出来给 -1，由插件说「要绕过去」。
 */
public final class BlockProjector {
    private static final int MAX_WALK_STEPS = 8;
    private static final int MAX_CLUSTER_CELLS = 64;
    private static final int MAX_BLOCKERS = 4;

    private BlockProjector() {
    }

    public static BlockProjection project(
            EntityMaid maid,
            LocalTerrain terrain,
            LocalTerrain mapTerrain,
            List<BlockSight> blocks,
            boolean withMap) {
        List<BlockRelation> relations = new ArrayList<>();
        List<BlockCluster> clusters = new ArrayList<>();
        for (BlockSight sight : blocks) {
            BlockRelation relation = relation(maid, terrain, sight);
            relations.add(relation);
            if (sight.seenNow()) {
                BlockCluster cluster = cluster(terrain, sight, relation);
                // 单块不算团：清单里已经有它，团只报连成片的
                if (cluster != null && cluster.cells() > 1) {
                    clusters.add(cluster);
                }
            }
        }
        MapProjection map = withMap && mapTerrain != null ? MapProjection.render(maid, mapTerrain) : null;
        return new BlockProjection(relations, clusters, map);
    }

    /** 视线中间隔着的实心方块有几块：格子寻址的失败回执用它说「中间隔了两块」。 */
    public static int blockerCount(Level level, Vec3 eye, BlockPos target) {
        return blockers(level, eye, target).count();
    }

    /** 挡在视线上的第一块方块 id；没有给空串。 */
    public static String blockerId(Level level, Vec3 eye, BlockPos target) {
        return blockers(level, eye, target).id();
    }

    private static BlockRelation relation(EntityMaid maid, LocalTerrain terrain, BlockSight sight) {
        BlockPos feet = maid.blockPosition();
        BlockPos pos = sight.pos();
        int dx = pos.getX() - feet.getX();
        int dy = pos.getY() - feet.getY();
        int dz = pos.getZ() - feet.getZ();
        String facing = Surroundings.direction(
                maid.getYRot(), pos.getX() - maid.getX(), pos.getZ() - maid.getZ());
        int blockers = 0;
        String blocker = "";
        if (!sight.seenNow()) {
            Blockers found = blockers(maid.level(), maid.getEyePosition(), pos);
            blockers = found.count();
            blocker = found.id();
        }
        int walkSteps = sight.seenNow() ? walkSteps(maid, terrain, pos) : -1;
        return new BlockRelation(
                sight.id(),
                sight.seenNow(),
                dx,
                dy,
                dz,
                facing,
                dy == 0,
                Math.sqrt(dx * dx + dz * dz),
                blockers,
                blocker,
                walkSteps);
    }

    /**
     * 八格内走到目标旁边大概几步。只做水平四向、上一格与同一层，遇到水与落差就停；
     * 这是「够不够得着」的直觉，不是寻路。
     */
    private static int walkSteps(EntityMaid maid, LocalTerrain terrain, BlockPos target) {
        BlockPos start = maid.blockPosition();
        int hx = target.getX() - start.getX();
        int hz = target.getZ() - start.getZ();
        if (hx * hx + hz * hz > MAX_WALK_STEPS * MAX_WALK_STEPS) {
            return -1;
        }
        Level level = maid.level();
        if (standable(level, terrain, start) && adjacent(start, target)) {
            return 0;
        }
        ArrayDeque<Step> queue = new ArrayDeque<>();
        Set<Long> seen = new HashSet<>();
        queue.add(new Step(start, 0));
        seen.add(start.asLong());
        int minY = start.getY();
        int maxY = start.getY() + 2;
        int[] offsets = {0, 1, -1};
        while (!queue.isEmpty()) {
            Step step = queue.poll();
            if (step.steps() >= MAX_WALK_STEPS) {
                continue;
            }
            for (Direction direction : Direction.Plane.HORIZONTAL) {
                int x = step.pos().getX() + direction.getStepX();
                int z = step.pos().getZ() + direction.getStepZ();
                for (int offset : offsets) {
                    int y = step.pos().getY() + offset;
                    if (y < minY || y > maxY) {
                        continue;
                    }
                    BlockPos next = new BlockPos(x, y, z);
                    if (!standable(level, terrain, next)) {
                        continue;
                    }
                    if (!seen.add(next.asLong())) {
                        continue;
                    }
                    int steps = step.steps() + 1;
                    if (adjacent(next, target)) {
                        return steps;
                    }
                    queue.add(new Step(next, steps));
                    break;
                }
            }
        }
        return -1;
    }

    private static boolean standable(Level level, LocalTerrain terrain, BlockPos pos) {
        BlockState feet = terrain.stateAt(pos.getX(), pos.getY(), pos.getZ());
        BlockState head = terrain.stateAt(pos.getX(), pos.getY() + 1, pos.getZ());
        BlockState below = terrain.stateAt(pos.getX(), pos.getY() - 1, pos.getZ());
        if (feet == null || head == null || below == null) {
            return false;
        }
        if (!feet.isAir() && !feet.canBeReplaced()) {
            return false;
        }
        if (!head.isAir() && !head.canBeReplaced()) {
            return false;
        }
        if (!feet.getFluidState().isEmpty() || !head.getFluidState().isEmpty()) {
            return false;
        }
        return !below.getCollisionShape(level, pos.below()).isEmpty();
    }

    private static boolean adjacent(BlockPos stand, BlockPos target) {
        int dx = Math.abs(stand.getX() - target.getX());
        int dy = Math.abs(stand.getY() - target.getY());
        int dz = Math.abs(stand.getZ() - target.getZ());
        return dx <= 1 && dy <= 1 && dz <= 1 && dx + dy + dz > 0;
    }

    /** 视线中间隔着的实心方块：只给「刚才见过」用，此刻看得见的一定是 0。 */
    private static Blockers blockers(Level level, Vec3 eye, BlockPos target) {
        Vec3 delta = Vec3.atCenterOf(target).subtract(eye);
        double length = delta.length();
        if (length < 0.01) {
            return new Blockers(0, "");
        }
        BlockPos eyePos = BlockPos.containing(eye.x, eye.y, eye.z);
        Set<Long> seen = new HashSet<>();
        int count = 0;
        String first = "";
        for (double step = 0.5; step < length; step += 0.5) {
            Vec3 point = eye.add(delta.scale(step / length));
            BlockPos pos = BlockPos.containing(point.x, point.y, point.z);
            if (pos.equals(target) || pos.equals(eyePos) || !seen.add(pos.asLong())) {
                continue;
            }
            BlockState state = level.getBlockState(pos);
            if (state.isAir() || state.getBlock() instanceof LiquidBlock) {
                continue;
            }
            if (state.getCollisionShape(level, pos).isEmpty()) {
                continue;
            }
            count++;
            if (first.isEmpty()) {
                first = BuiltInRegistries.BLOCK.getKey(state.getBlock()).toString();
            }
            if (count >= MAX_BLOCKERS) {
                break;
            }
        }
        return new Blockers(count, first);
    }

    /** 从最近那一格往六面扩，数一数这一团同种方块有多少格。 */
    private static BlockCluster cluster(
            LocalTerrain terrain, BlockSight sight, BlockRelation relation) {
        BlockState start = terrain.stateAt(
                sight.pos().getX(), sight.pos().getY(), sight.pos().getZ());
        if (start == null) {
            return null;
        }
        Block block = start.getBlock();
        ArrayDeque<BlockPos> queue = new ArrayDeque<>();
        Set<Long> seen = new HashSet<>();
        queue.add(sight.pos());
        seen.add(sight.pos().asLong());
        int cells = 0;
        boolean truncated = false;
        while (!queue.isEmpty()) {
            if (cells >= MAX_CLUSTER_CELLS) {
                truncated = true;
                break;
            }
            BlockPos pos = queue.poll();
            cells++;
            for (Direction direction : Direction.values()) {
                BlockPos next = pos.relative(direction);
                if (!seen.add(next.asLong())) {
                    continue;
                }
                BlockState state = terrain.stateAt(next.getX(), next.getY(), next.getZ());
                if (state == null || state.getBlock() != block) {
                    continue;
                }
                queue.add(next);
            }
        }
        return new BlockCluster(
                sight.id(),
                cells,
                truncated,
                relation.dx(),
                relation.dy(),
                relation.dz(),
                relation.facing(),
                relation.distance(),
                relation.blockers(),
                relation.blocker());
    }

    private record Step(BlockPos pos, int steps) {
    }

    private record Blockers(int count, String id) {
    }
}
