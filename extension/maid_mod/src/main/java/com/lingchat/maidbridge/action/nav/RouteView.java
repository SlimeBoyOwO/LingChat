package com.lingchat.maidbridge.action.nav;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.place.Placement;
import java.util.HashMap;
import java.util.Map;
import java.util.function.Predicate;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.DoorBlock;
import net.minecraft.world.level.block.state.BlockState;

/**
 * 让落脚点图搜索看见 Minecraft 的世界：能不能站、能不能放、哪一跳落得过去。
 *
 * <p>判据与执行侧共用同一套地形口径（{@link Terrain}、{@link MaidParkour}、
 * {@link Placement}），所以搜索认为行的路，腿去走的时候大概率也成立。站位的视线仍按
 * 站到格心后的理想眼睛算；到站之后眼睛偏一点的事由放置链自己兜。
 */
final class RouteView implements RouteSearch.View {
    private final EntityMaid maid;
    private final ServerLevel level;
    private final BlockPos target;
    private final Predicate<BlockPos> goal;
    private final NavPolicy policy;
    private final NavMemory memory;
    /** 她身上有没有能垫的整方块：没有就一条垫脚边都不连。 */
    private final boolean hasScaffold;
    /** 一次搜索内的站位缓存：跑跳要试一圈落点，同一格会被反复问到。 */
    private final Map<BlockPos, Boolean> stands = new HashMap<>();

    RouteView(EntityMaid maid, BlockPos target, Predicate<BlockPos> goal,
            NavPolicy policy, NavMemory memory) {
        this.maid = maid;
        this.level = (ServerLevel) maid.level();
        this.target = target;
        this.goal = goal;
        this.policy = policy;
        this.memory = memory;
        this.hasScaffold = ScaffoldBook.item(maid, policy.consumeMaterials()) != null;
    }

    @Override
    public boolean standable(BlockPos pos) {
        // 试过站不上去的格子不再当落脚点，免得在同一格上反复走
        return stands.computeIfAbsent(pos.immutable(), key -> allowed(key)
                && !memory.triedStands.contains(key)
                && Terrain.standable(level, key));
    }

    @Override
    public boolean air(BlockPos pos) {
        return Terrain.free(level, pos);
    }

    @Override
    public boolean solid(BlockPos pos) {
        return Terrain.solid(level, pos);
    }

    @Override
    public boolean passable(BlockPos pos) {
        // 木门算能过：原版导航会替她推开；铁门推不动。水能游过去，不挡路
        return allowed(pos) && (standable(pos) || door(pos) || swim(pos));
    }

    /** 这一格能游过去：脚下是水，头顶没有实心方块。 */
    private boolean swim(BlockPos pos) {
        return Terrain.water(level, pos) && !Terrain.solid(level, pos.above());
    }

    @Override
    public boolean breakable(BlockPos pos) {
        // 图纸格不许拆；拆得动的实心方块才行，水和空气谈不上拆
        return allowed(pos)
                && policy.canBreak(pos)
                && !memory.badBlocks.contains(pos)
                && !Terrain.free(level, pos)
                && !Terrain.water(level, pos)
                && level.getBlockState(pos).getDestroySpeed(level, pos) >= 0;
    }

    /** 居家模式的活动范围：图上的路也不能越过它，否则原版走到边界就会停。 */
    private boolean allowed(BlockPos pos) {
        return !maid.hasRestriction() || maid.isWithinRestriction(pos);
    }

    private boolean door(BlockPos pos) {
        BlockState state = level.getBlockState(pos);
        return state.getBlock() instanceof DoorBlock && state.getBlock() != Blocks.IRON_DOOR;
    }

    @Override
    public boolean placeable(BlockPos block, BlockPos from) {
        if (!hasScaffold
                || !allowed(block)
                || !policy.canBuildOn(block)
                || !Terrain.free(level, block)
                || memory.badBlocks.contains(block)
                || Bodies.other(level, block, maid)) {
            return false;
        }
        if (!Terrain.reachableFrom(maid, from, block)) {
            return false;
        }
        // 她脚下踩着的那块就是托面：架桥、往上垫时，下一块贴着它放就行。搜索里还看不见
        // 这条路上刚铺出来的方块，真到执行的时候它们已经在了
        if (block.distManhattan(from.below()) == 1) {
            return true;
        }
        return Placement.seesFrom(maid, from, block, null);
    }

    @Override
    public boolean fits(BlockPos stand, BlockPos target) {
        return !Bodies.selfAt(stand, target, maid);
    }

    @Override
    public boolean hop(BlockPos from, BlockPos to, boolean builtFloor) {
        // 落点先过缓存：一圈落点大多站不住，不必每个都走一遍弧线校验
        return standable(to)
                && MaidParkour.canHop(level, from, to, ParkourRules.MAX_FALL, builtFloor);
    }

    @Override
    public boolean drop(BlockPos from, BlockPos to, boolean builtFloor) {
        return standable(to)
                && MaidParkour.canHop(level, from, to, ParkourRules.DEEP_FALL, builtFloor);
    }

    @Override
    public boolean goal(BlockPos pos) {
        // 站不站得住由搜索按这一步有没有铺出脚下的方块来判，这里只回答「到没到」
        return !memory.triedStands.contains(pos) && goal.test(pos);
    }
}
