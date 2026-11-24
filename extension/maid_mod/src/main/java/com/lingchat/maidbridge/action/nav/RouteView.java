package com.lingchat.maidbridge.action.nav;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.place.Placement;
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
    private final NavGoal.Stand goal;
    private final NavPolicy policy;
    private final NavMemory memory;
    private final BlockPos target;

    RouteView(EntityMaid maid, NavGoal.Stand goal, NavPolicy policy, NavMemory memory) {
        this.maid = maid;
        this.level = (ServerLevel) maid.level();
        this.goal = goal;
        this.policy = policy;
        this.memory = memory;
        this.target = goal.target();
    }

    @Override
    public boolean standable(BlockPos pos) {
        // 试过站不上去的格子不再当落脚点，免得在同一格上反复走
        return !memory.triedStands.contains(pos) && Terrain.standable(level, pos);
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
        // 木门算能过：原版导航会替她推开；铁门推不动
        return standable(pos) || door(pos);
    }

    private boolean door(BlockPos pos) {
        BlockState state = level.getBlockState(pos);
        return state.getBlock() instanceof DoorBlock && state.getBlock() != Blocks.IRON_DOOR;
    }

    @Override
    public boolean placeable(BlockPos block, BlockPos from) {
        if (!policy.canBuildOn(block)
                || !Terrain.free(level, block)
                || memory.badBlocks.contains(block)
                || Bodies.other(level, block, maid)) {
            return false;
        }
        return Terrain.reachableFrom(maid, from, block)
                && Placement.seesFrom(maid, from, block, null);
    }

    @Override
    public boolean fits(BlockPos stand, BlockPos target) {
        return !Bodies.selfAt(stand, target, maid);
    }

    @Override
    public boolean hop(BlockPos from, BlockPos to) {
        return !memory.triedStands.contains(to)
                && MaidParkour.canHop(level, from, to, ParkourRules.MAX_FALL);
    }

    @Override
    public boolean drop(BlockPos from, BlockPos to) {
        return !memory.triedStands.contains(to)
                && MaidParkour.canHop(level, from, to, ParkourRules.DEEP_FALL);
    }

    @Override
    public boolean goal(BlockPos pos) {
        // 站不站得住由搜索按这一步有没有铺出脚下的方块来判，这里只回答「能不能动手」
        return !pos.equals(target)
                && !memory.triedStands.contains(pos)
                && goal.canActFrom().test(pos);
    }
}
