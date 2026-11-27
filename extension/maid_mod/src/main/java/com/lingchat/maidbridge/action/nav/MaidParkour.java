package com.lingchat.maidbridge.action.nav;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.core.MaidActuator;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import java.util.function.Predicate;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.Vec3;

/**
 * 跑酷：一段冲刺跳比垫一块方块便宜，能跳过去就不搭。
 *
 * <p>每一步都先算清落点：起跳格与落点格都要站得下，飞行包络里不能有实心方块，
 * 断口下面三格内不能是岩浆、火这类掉下去就麻烦的方块。深谷可以跳，落点是提前验过的。
 */
public final class MaidParkour {
    /** 一条路线最多几次跑跳：够跨过一串单格跳台，又不会把一次施工耗在路上。 */
    private static final int MAX_HOPS = 5;
    /** 搜索最多展开这么多个落脚点，避免一格要算上万次。 */
    private static final int MAX_NODES = 24;
    /** 弧顶比直线抬高多少：看起来像一次跳，也够越过一格高的檐口。 */
    private static final double ARC_HEIGHT = 1.05;
    /** 弧线上身体会比落脚面高出一格多，飞行包络要多查两层。 */
    private static final int ARC_CLEARANCE = 2;
    /** 半个身宽，略小于 0.3：贴着格子边飞的时候不把隔壁一列算进来。 */
    private static final double HALF_WIDTH = 0.29;

    private MaidParkour() {
    }

    /**
     * 从她脚下出发，用至多几次跑跳找一个满足条件的落脚点。
     *
     * <p>返回要依次踩的格子；空列表表示跑跳帮不上忙。只走直线、只沿四个方向，
     * 与沙盘里的规则一致。中间落点在 {@code skip} 里的不再选，避免反复试同一条死路。
     */
    public static List<BlockPos> planTo(
            EntityMaid maid, Set<BlockPos> skip, Predicate<BlockPos> standOk, int maxFall) {
        if (!(maid.level() instanceof ServerLevel level) || !maid.onGround()) {
            return List.of();
        }
        BlockPos start = maid.blockPosition();
        if (!Terrain.standable(level, start)) {
            return List.of();
        }
        Set<BlockPos> seen = new HashSet<>();
        seen.add(start);
        ArrayDeque<Node> queue = new ArrayDeque<>();
        queue.add(new Node(start, null));
        int nodes = 0;
        while (!queue.isEmpty() && nodes++ < MAX_NODES) {
            Node node = queue.poll();
            if (!node.pos.equals(start) && !skip.contains(node.pos) && standOk.test(node.pos)) {
                return chainOf(node);
            }
            if (node.depth >= MAX_HOPS) {
                continue;
            }
            for (BlockPos next : hopLandings(level, maid, node.pos, maxFall)) {
                if (skip.contains(next) || !seen.add(next)) {
                    continue;
                }
                queue.add(new Node(next, node));
            }
        }
        return List.of();
    }

    /** 两次跑跳之间能落的格子。 */
    private static List<BlockPos> hopLandings(
            ServerLevel level, EntityMaid maid, BlockPos from, int maxFall) {
        List<BlockPos> out = new ArrayList<>();
        for (Direction dir : Direction.Plane.HORIZONTAL) {
            for (int distance = 1; distance <= ParkourRules.MAX_DISTANCE; distance++) {
                for (int rise = ParkourRules.MAX_RISE; rise >= -maxFall; rise--) {
                    if (!ParkourRules.allowed(distance, rise, maxFall)) {
                        continue;
                    }
                    BlockPos to = from.offset(
                            dir.getStepX() * distance, rise, dir.getStepZ() * distance);
                    if (!level.isLoaded(to) || !level.isLoaded(to.above(ARC_CLEARANCE))) {
                        continue;
                    }
                    if (maid.hasRestriction() && !maid.isWithinRestriction(to)) {
                        continue;
                    }
                    if (canHop(level, from, to, maxFall)) {
                        out.add(to);
                    }
                }
            }
        }
        return out;
    }

    /** 这一跳能不能跳：距离、高差、落点、飞行包络与断口深度一起判。 */
    public static boolean canHop(ServerLevel level, BlockPos from, BlockPos to, int maxFall) {
        return canHop(level, from, to, maxFall, false);
    }

    /**
     * 同上；builtFloor 为真时起跳格脚下是这条路线上还没放下的垫脚块，只验脚和头空着。
     *
     * <p>落点可以斜着、错位：飞行包络按身体从起跳格心飞到落点格心扫过的格子列来查。
     */
    public static boolean canHop(
            ServerLevel level, BlockPos from, BlockPos to, int maxFall, boolean builtFloor) {
        int rise = to.getY() - from.getY();
        int dx = to.getX() - from.getX();
        int dz = to.getZ() - from.getZ();
        if (!ParkourRules.allowed(dx, dz, rise, maxFall)) {
            return false;
        }
        boolean takeoff = builtFloor
                ? Terrain.free(level, from) && Terrain.free(level, from.above())
                : Terrain.standable(level, from);
        if (!takeoff || !Terrain.standable(level, to)) {
            return false;
        }
        // 落点要落在整格的面上：台阶、栅栏、半砖的顶面高度对不上，摆过去会卡在碰撞盒里
        if (!fullFloor(level, to)) {
            return false;
        }
        // 起跳与落地时头顶都要多让出一格；落点上方还要留出弧线掠过的高度
        if (!clear(level, from.above(2))
                || !clear(level, to.above(2))
                || !clear(level, to.above(3))) {
            return false;
        }
        boolean deep = rise < -ParkourRules.MAX_FALL;
        if (deep) {
            // 深落是自己摆位置往下走：落点上方的整根柱子要空到起跳高度，不能从屋檐里穿下去
            for (int y = to.getY() + 1; y <= from.getY() + ARC_CLEARANCE; y++) {
                if (!clear(level, to.atY(y))) {
                    return false;
                }
            }
        }
        int minY = Math.min(from.getY(), to.getY());
        int maxY = Math.max(from.getY(), to.getY());
        boolean gap = false;
        for (BlockPos cell : sweptColumns(from, to)) {
            for (int y = minY; y <= maxY + ARC_CLEARANCE; y++) {
                if (!clear(level, cell.atY(y))) {
                    return false;
                }
            }
            if (!Terrain.standable(level, cell)) {
                gap = true;
                // 断口下面不能是岩浆、火这类；近处有危险就宁可垫方块
                if (!safeBelow(level, cell, from.getY(), maxFall)) {
                    return false;
                }
            }
        }
        // 地面连续时走过去更省，不生成这种空跳；深落是往下走，不要求中间有断口
        return gap || deep;
    }

    /**
     * 身体从起跳格心直线飞到落点格心时扫过的格子列，高度取起跳格，不含起落两列。
     *
     * <p>沿中线和左右各偏半个身宽的两条线取样：斜着跳时擦过的墙角也算在里面。
     */
    static Set<BlockPos> sweptColumns(BlockPos from, BlockPos to) {
        double dx = to.getX() - from.getX();
        double dz = to.getZ() - from.getZ();
        double length = Math.sqrt(dx * dx + dz * dz);
        Set<BlockPos> out = new LinkedHashSet<>();
        if (length < 1.0E-6) {
            return out;
        }
        double sideX = -dz / length * HALF_WIDTH;
        double sideZ = dx / length * HALF_WIDTH;
        BlockPos landing = to.atY(from.getY());
        int samples = (int) Math.ceil(length * 8);
        for (int i = 1; i < samples; i++) {
            double s = i / (double) samples;
            double x = from.getX() + 0.5 + dx * s;
            double z = from.getZ() + 0.5 + dz * s;
            for (int side = -1; side <= 1; side++) {
                BlockPos cell = BlockPos.containing(
                        x + sideX * side, from.getY(), z + sideZ * side);
                if (!cell.equals(from) && !cell.equals(landing)) {
                    out.add(cell);
                }
            }
        }
        return out;
    }

    /** 落点底下是不是整格方块：台阶、栅栏、半砖不能当跑跳的落脚台。 */
    private static boolean fullFloor(ServerLevel level, BlockPos stand) {
        BlockPos floor = stand.below();
        return level.getBlockState(floor).isCollisionShapeFullBlock(level, floor);
    }

    /** 这一格是不是空的：没有碰撞体积，也不是液体。 */
    private static boolean clear(ServerLevel level, BlockPos pos) {
        BlockState state = level.getBlockState(pos);
        if (!state.getFluidState().isEmpty()) {
            return false;
        }
        return state.getCollisionShape(level, pos).isEmpty();
    }

    /**
     * 断口下面三格内不能是液体、岩浆、火这类掉下去就麻烦的方块。
     *
     * <p>深谷可以跳：弧线是我们自己按落点摆的，半路不会掉下去；但近处要是有岩浆，
     * 一旦被打断就救不回来，宁可改去垫方块。
     */
    private static boolean safeBelow(ServerLevel level, BlockPos cell, int fromY, int maxFall) {
        for (int y = fromY - 1; y >= fromY - 1 - maxFall; y--) {
            BlockPos pos = cell.atY(y);
            if (!level.isLoaded(pos)) {
                return false;
            }
            BlockState state = level.getBlockState(pos);
            if (state.getBlock() instanceof LiquidBlock) {
                return false;
            }
            if (state.getCollisionShape(level, pos).isEmpty()) {
                continue;
            }
            return !dangerous(state.getBlock());
        }
        return true;
    }

    /** 踩上去会受伤的方块：掉下去也算没接住。 */
    private static boolean dangerous(Block block) {
        return block == Blocks.FIRE
                || block == Blocks.SOUL_FIRE
                || block == Blocks.MAGMA_BLOCK
                || block == Blocks.CACTUS
                || block == Blocks.SWEET_BERRY_BUSH
                || block == Blocks.POWDER_SNOW
                || block == Blocks.CAMPFIRE
                || block == Blocks.SOUL_CAMPFIRE;
    }

    /** 从终点沿着父节点倒回起跳点；起跳点本身不放进路线。 */
    private static List<BlockPos> chainOf(Node node) {
        List<BlockPos> chain = new ArrayList<>();
        for (Node n = node; n.parent != null; n = n.parent) {
            chain.add(0, n.pos);
        }
        return chain;
    }

    private static final class Node {
        final BlockPos pos;
        final Node parent;
        final int depth;

        Node(BlockPos pos, Node parent) {
            this.pos = pos;
            this.parent = parent;
            this.depth = parent == null ? 0 : parent.depth + 1;
        }
    }

    /**
     * 一段跑跳的执行器：每 tick 把位置摆到弧线上一点，走完一段再起下一段。
     *
     * <p>不用物理速度推：女仆的寻路与任务每 tick 都可能写速度，靠冲量很难稳。
     * 先按包络校验过没有实心方块，再自己摆位置，落点就是准的；途中关掉重力、
     * 清掉速度，结束或收手时再还给她。
     */
    public static final class Runner {
        private final EntityMaid maid;
        private final List<BlockPos> chain;
        private final int maxFall;
        private int index;
        private int ticks;
        private int total;
        private BlockPos from;
        private Vec3 start;
        private Vec3 end;
        private boolean done;
        private boolean failed;

        public Runner(EntityMaid maid, List<BlockPos> chain, int maxFall) {
            this.maid = maid;
            this.chain = List.copyOf(chain);
            this.maxFall = maxFall;
            if (!(maid.level() instanceof ServerLevel level)
                    || this.chain.isEmpty()
                    || !maid.onGround()) {
                failed = true;
                restore();
                return;
            }
            this.from = maid.blockPosition();
            if (!canHop(level, from, this.chain.get(0), maxFall)) {
                failed = true;
                restore();
                return;
            }
            MaidActuator.stopWalking(maid);
            maid.setNoGravity(true);
            maid.setDeltaMovement(Vec3.ZERO);
            maid.setSprinting(true);
            beginLeg();
        }

        private void beginLeg() {
            BlockPos to = chain.get(index);
            // 起点按她实际的脚高取：站在台阶上时脚不在整格顶面
            start = new Vec3(from.getX() + 0.5, maid.getY(), from.getZ() + 0.5);
            end = Vec3.atBottomCenterOf(to);
            ticks = 0;
            total = ParkourRules.flightTicks(to.getY() - from.getY());
            maid.setPos(start.x, start.y, start.z);
            MaidActuator.lookAtPos(maid, to);
        }

        public void tick() {
            if (done || failed) {
                return;
            }
            if (!(maid.level() instanceof ServerLevel level) || !maid.isAlive()) {
                fail();
                return;
            }
            BlockPos to = chain.get(index);
            if (!Terrain.standable(level, to)) {
                fail();
                return;
            }
            // 飞的时候不许别的行为写寻路目标，否则她在下一 tick 会被拽走
            MaidActuator.stopWalking(maid);
            ticks++;
            double s = Math.min(1.0, ticks / (double) total);
            double x = start.x + (end.x - start.x) * s;
            double z = start.z + (end.z - start.z) * s;
            double y = start.y + (end.y - start.y) * s + ARC_HEIGHT * Math.sin(Math.PI * s);
            maid.setPos(x, y, z);
            maid.setDeltaMovement(Vec3.ZERO);
            if (s < 1.0) {
                return;
            }
            maid.setPos(end.x, end.y, end.z);
            index++;
            from = to;
            if (index >= chain.size()) {
                done = true;
                restore();
                return;
            }
            BlockPos next = chain.get(index);
            if (!canHop(level, from, next, maxFall)) {
                fail();
                return;
            }
            beginLeg();
        }

        public boolean done() {
            return done;
        }

        public boolean failed() {
            return failed;
        }

        public List<BlockPos> chain() {
            return chain;
        }

        /** 收手：从弧线上退回这一段的起跳点，别把人留在半空。 */
        public void cancel() {
            if (done || failed) {
                return;
            }
            done = true;
            if (start != null) {
                maid.setPos(start.x, start.y, start.z);
            }
            restore();
        }

        private void fail() {
            failed = true;
            if (start != null) {
                maid.setPos(start.x, start.y, start.z);
            }
            restore();
        }

        private void restore() {
            maid.setNoGravity(false);
            maid.setSprinting(false);
            maid.setDeltaMovement(Vec3.ZERO);
        }
    }
}
