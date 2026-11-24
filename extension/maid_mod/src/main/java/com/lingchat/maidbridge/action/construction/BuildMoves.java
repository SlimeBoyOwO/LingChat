package com.lingchat.maidbridge.action.construction;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.construction.BuildRules.Failure;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.action.mine.MineRules;
import com.lingchat.maidbridge.action.mine.MineTargets;
import com.lingchat.maidbridge.action.place.Placement;
import com.lingchat.maidbridge.maid.MaidInventory;
import com.lingchat.maidbridge.perception.item.ItemReporter;
import java.util.List;
import java.util.Map;
import java.util.function.Predicate;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;
import net.neoforged.neoforge.items.wrapper.CombinedInvWrapper;

import static com.lingchat.maidbridge.action.construction.BuildLayer.*;
import static com.lingchat.maidbridge.action.construction.BuildPlanner.*;
import com.lingchat.maidbridge.action.construction.BuildRules.*;
import com.lingchat.maidbridge.action.construction.MaidBuildController.Cell;
import com.lingchat.maidbridge.action.construction.MaidBuildController.Run;
import static com.lingchat.maidbridge.action.construction.MaidBuildController.*;

/** 走动与垫脚：同一时刻只有一个动作在跑。 */
final class BuildMoves {
    /**
     * 同一时刻只允许一个动作在跑：走路、垫脚、起跳垫高、跑跳。
     *
     * <p>以前这些状态散在几个可空字段里，优先级取决于 tick 里 if 的书写顺序，
     * 两个动作同时活着时就会互相抢。现在只由 {@link Run#move} 一个字段持有。
     */
    interface Move {
        enum Status {
            BUSY,
            DONE,
            FAILED
        }

        /** 推进一 tick。结束（完成或失败）前可以顺手发起下一个动作。 */
        Status tick(Run run, EntityMaid maid);

        /** 这个动作正为哪一格服务。 */
        Cell cell();

        /** 调试栏里的阶段名。 */
        String phase();

        /** 她正赶过去的落脚点，没有就 null。 */
        default BlockPos stand() {
            return null;
        }

        /** 下一块大概会放的位置。 */
        default BlockPos nextPlace() {
            return null;
        }

        /** 跑跳路线，调试可视化用。 */
        default List<BlockPos> chain() {
            return List.of();
        }

        /** 中途取消：不讲究落脚点，直接放手。 */
        default void cancel(Run run, EntityMaid maid) {
        }
    }

    /** 走到落脚点：到了就试着放这一格。 */
    static final class WalkMove implements Move {
        final Cell cell;
        final BlockPos stand;
        /** 必须真的踩进那一格：差一格就判到站，她会从斜角起跳。 */
        final boolean tight;
        /** 到站之后立刻试放这一格；平着搭路只是先走近，到站交给下个 tick 重新判。 */
        final boolean placeOnArrival;
        int ticks;
        int still;
        Vec3 last;

        WalkMove(EntityMaid maid, Cell cell, BlockPos stand, boolean tight) {
            this(maid, cell, stand, tight, true);
        }

        WalkMove(EntityMaid maid, Cell cell, BlockPos stand, boolean tight, boolean placeOnArrival) {
            this.cell = cell;
            this.stand = stand;
            this.tight = tight;
            this.placeOnArrival = placeOnArrival;
            this.last = maid.position();
            MaidActuator.walkToPos(maid, stand, 0);
        }

        @Override
        public Status tick(Run run, EntityMaid maid) {
            if (arrived(stand, maid, tight)) {
                // 先放下手上的动作，再判这一格：放置失败时新的站位才能顶上来，
                // 调试栏的「下一块」也不会错指到刚放下的这一格
                run.move = null;
                if (placeOnArrival) {
                    tryPlace(run, maid, cell);
                }
                return Status.DONE;
            }
            // 绕路时离目标变远是正常的，只看她自己有没有真的在动
            Vec3 now = maid.position();
            if (last != null && now.distanceToSqr(last) < 0.0025) {
                still++;
            } else {
                still = 0;
            }
            last = now;
            boolean noPath = ++ticks > STUCK_TICKS
                    && maid.getBrain().hasMemoryValue(MemoryModuleType.CANT_REACH_WALK_TARGET_SINCE);
            if (noPath || still > WALK_STILL_TICKS) {
                cell.triedStands.add(stand);
                // 这一格没走到：下个 tick 先换站位、跑跳或垫脚再试它，别直接扫下一格去
                run.active = cell;
                run.cooldown = PASS_BREAK_TICKS;
                return Status.FAILED;
            }
            MaidActuator.walkToPos(maid, stand, 0);
            return Status.BUSY;
        }

        @Override
        public Cell cell() {
            return cell;
        }

        @Override
        public BlockPos stand() {
            return stand;
        }

        @Override
        public BlockPos nextPlace() {
            return cell.pos;
        }

        @Override
        public String phase() {
            return "走向落脚点";
        }
    }

    /** 垫好托底之后，把顶块放上再走上去。 */
    static final class StepMove implements Move {
        final Cell cell;
        final BlockPos top;
        final Item item;

        StepMove(Cell cell, BlockPos top, Item item) {
            this.cell = cell;
            this.top = top;
            this.item = item;
        }

        @Override
        public Status tick(Run run, EntityMaid maid) {
            if (cell.steps >= ScaffoldRules.MAX_STEPS
                    || !stepRoom(run, maid, top) || !put(run, maid, cell, top, item)) {
                return Status.FAILED;
            }
            cell.steps++;
            climbOnto(run, maid, cell, top.above());
            return Status.DONE;
        }

        @Override
        public Cell cell() {
            return cell;
        }

        @Override
        public BlockPos nextPlace() {
            return top;
        }

        @Override
        public String phase() {
            return "垫脚：放顶块";
        }
    }

    /** 原地跳起来把脚下的方块补上，落下来就升了一整格。 */
    static final class JumpMove implements Move {
        final Cell cell;
        /** 她起跳前站的那一格，方块就补在这里。 */
        final BlockPos pos;
        final Item item;
        int ticks;

        JumpMove(Cell cell, BlockPos pos, Item item) {
            this.cell = cell;
            this.pos = pos;
            this.item = item;
        }

        @Override
        public Status tick(Run run, EntityMaid maid) {
            if (++ticks > JUMP_TIMEOUT_TICKS) {
                // 跳不起来（头顶被封、被别的模组按住）：这一格换别的办法
                CellFlow.hold(run, cell, Failure.NO_STAND);
                return Status.FAILED;
            }
            if (maid.onGround()) {
                if (!maid.blockPosition().equals(pos) || ticks > 3) {
                    // 被挤开了，或者推了几下都没离地
                    CellFlow.hold(run, cell, Failure.NO_STAND);
                    return Status.FAILED;
                }
                maid.setDeltaMovement(0.0, JUMP_POWER, 0.0);
                return Status.BUSY;
            }
            if (maid.getY() < pos.getY() + 1.02) {
                return Status.BUSY;
            }
            if (!put(run, maid, cell, pos, item)) {
                CellFlow.hold(run, cell, Failure.PLACE_FAILED);
                return Status.FAILED;
            }
            cell.steps++;
            run.cooldown = JUMP_LAND_TICKS;
            return Status.DONE;
        }

        @Override
        public Cell cell() {
            return cell;
        }

        @Override
        public BlockPos nextPlace() {
            return pos;
        }

        @Override
        public String phase() {
            return "原地起跳垫高";
        }
    }

    /** 跑跳跨过缺口：落点必须够得着、看得见这一格。 */
    static final class HopMove implements Move {
        final Cell cell;
        final MaidParkour.Runner runner;
        /** 落地后立刻试放；只为下到地面而跳时不必试，交给下一 tick 重新找路。 */
        final boolean placeAfter;

        HopMove(Cell cell, MaidParkour.Runner runner) {
            this(cell, runner, true);
        }

        HopMove(Cell cell, MaidParkour.Runner runner, boolean placeAfter) {
            this.cell = cell;
            this.runner = runner;
            this.placeAfter = placeAfter;
        }

        @Override
        public Status tick(Run run, EntityMaid maid) {
            runner.tick();
            if (runner.failed()) {
                // 这条路线走不通：记下来，换别的落点或者退回垫脚
                cell.triedStands.addAll(runner.chain());
                CellFlow.hold(run, cell, Failure.NO_PATH);
                run.cooldown = PASS_BREAK_TICKS;
                return Status.FAILED;
            }
            if (!runner.done()) {
                return Status.BUSY;
            }
            run.cooldown = PASS_BREAK_TICKS;
            LOGGER.info("施工跑跳：{} 跳后落在 {}", runner.chain().size(), maid.blockPosition());
            // 站定之后就是放这一格；失败时 triedStands 会记住这个落点，下一轮换别处
            run.move = null;
            run.stand = maid.blockPosition();
            if (placeAfter) {
                tryPlace(run, maid, cell);
            }
            return Status.DONE;
        }

        @Override
        public Cell cell() {
            return cell;
        }

        @Override
        public String phase() {
            return "跑跳过缺口";
        }

        @Override
        public List<BlockPos> chain() {
            return runner.chain();
        }

        @Override
        public void cancel(Run run, EntityMaid maid) {
            runner.cancel();
        }
    }

    /** 推进手上唯一的那个动作；它结束时自己清空，接下一个动作时直接替换。 */
    static void advance(Run run, EntityMaid maid) {
        Move move = run.move;
        if (move == null) {
            return;
        }
        if (move.tick(run, maid) != Move.Status.BUSY && run.move == move) {
            run.move = null;
        }
    }

    /** 换一个站位：先走到那一格，到了再放。 */
    static void startWalk(Run run, EntityMaid maid, Cell cell, BlockPos stand) {
        run.stand = stand;
        run.standFor = cell;
        run.nextPlaceIn = 0;
        run.move = new WalkMove(maid, cell, stand, true);
    }

    /** 只是往目标挪一格：到站不再试放，下个 tick 按新位置重新判这一格。 */
    static void walkOnly(Run run, EntityMaid maid, Cell cell, BlockPos stand) {
        run.stand = stand;
        run.standFor = cell;
        run.nextPlaceIn = 0;
        run.move = new WalkMove(maid, cell, stand, true, false);
    }

    /** 放下手上的动作；中途取消时不讲究落脚点。 */
    static void cancelMove(Run run, EntityMaid maid) {
        Move move = run.move;
        run.move = null;
        if (move != null) {
            move.cancel(run, maid);
        }
    }

    /** 在屋里被天花板顶住时，先去附近一处露天的地方，再沿外墙往上垫。 */
    static boolean stageOutside(Run run, EntityMaid maid, Cell cell) {
        if (run.level.canSeeSky(maid.blockPosition().above())) {
            return false;
        }
        BlockPos stage = MineTargets.stageFor(maid, cell.pos, run.search);
        if (stage == null) {
            return false;
        }
        startWalk(run, maid, cell, stage);
        return true;
    }

    /** 她此刻能不能起跳：脚与头两格之外，头顶上面还要再让出一格。 */
    static boolean canJumpHere(Run run, EntityMaid maid) {
        BlockPos feet = maid.blockPosition();
        return freeForStep(run, feet.above()) && freeForStep(run, feet.above(2));
    }

    /**
     * 自己正站在目标格里、周围又没有实地可站：在旁边的空中搭一格，站过去再放。
     *
     * <p>屋顶、墙头这类地方，脚下这块就是建筑本身，边上直接是空的。把与脚下同一层的
     * 邻格垫出来（要有依托、看得见才放），走过去，目标格就空出来了；下一 tick 再放。
     */
    static boolean stepAside(Run run, EntityMaid maid, Cell cell) {
        if (cell.steps >= ScaffoldRules.MAX_STEPS
                || run.scaffold.size() >= ScaffoldRules.MAX_BLOCKS) {
            return false;
        }
        BlockPos feet = maid.blockPosition();
        if (!canJumpHere(run, maid)) {
            return false;
        }
        Item item = scaffoldItem(run, maid);
        if (item == null) {
            return false;
        }
        for (Direction side : Direction.Plane.HORIZONTAL) {
            BlockPos stand = feet.relative(side);
            // 图格上不能站人，那就不是「让开」而是又占一格
            if (run.blueprint.contains(stand) || cell.triedStands.contains(stand)) {
                continue;
            }
            if (!freeForStep(run, stand)
                    || !freeForStep(run, stand.above())
                    || selfInside(stand, maid)
                    || otherEntityInside(run, stand, maid)) {
                continue;
            }
            if (solidCell(run.level, stand.below())) {
                walkOnly(run, maid, cell, stand);
                return true;
            }
            BlockPos floor = stand.below();
            if (run.blueprint.contains(floor)
                    || !freeForStep(run, floor)
                    || !withinReach(maid, floor)
                    || otherEntityInside(run, floor, maid)
                    || !put(run, maid, cell, floor, item)) {
                continue;
            }
            cell.steps++;
            run.cooldown = PASS_BREAK_TICKS;
            walkOnly(run, maid, cell, stand);
            return true;
        }
        return false;
    }

    /**
     * 目标在她下面：先跳到目标附近的地面，落地之后再按正常流程寻路。
     *
     * <p>女仆的寻路不跳崖，站在屋顶、墙头或者自己搭的柱子上时，墙根、地基那几格
     * 无论从哪算都到不了。落点只要求站得住、比她低、离目标不远，不要求当场放得下；
     * 落到地面以后，选站位与视线都按平地上的距离重新算，接下来交给普通走路。
     */
    static boolean dropDown(Run run, EntityMaid maid, Cell cell) {
        if (run.move != null || !maid.onGround() || cell.drops >= MAX_DROPS) {
            return false;
        }
        if (cell.hopUntil > run.level.getGameTime()) {
            return false;
        }
        // 落在实地（不是自己垫的方块）上：目标附近、比她低、不超过四格
        Predicate<BlockPos> landing = stand -> stand.getY() <= cell.pos.getY() + 1
                && Math.abs(stand.getX() - cell.pos.getX()) <= 4
                && Math.abs(stand.getZ() - cell.pos.getZ()) <= 4
                && !run.scaffold.containsKey(stand.below());
        List<BlockPos> chain = MaidParkour.planToPlace(
                maid, cell.pos, cell.triedStands, landing, ParkourRules.DEEP_FALL);
        if (chain.isEmpty()) {
            cell.hopUntil = run.level.getGameTime() + HOP_RETRY_TICKS;
            return false;
        }
        run.stand = null;
        MaidParkour.Runner runner = new MaidParkour.Runner(maid, chain, ParkourRules.DEEP_FALL);
        if (runner.failed()) {
            cell.triedStands.addAll(chain);
            cell.hopUntil = run.level.getGameTime() + HOP_RETRY_TICKS;
            return false;
        }
        cell.drops++;
        // 落到地面以后，是从高处试出来的那批站位都不作数了，重新找
        cell.triedStands.clear();
        cell.waits = 0;
        LOGGER.info("施工下落：为 {} 跳到 {}", cell.pos, chain.get(chain.size() - 1));
        run.move = new HopMove(cell, runner, false);
        return true;
    }

    /** 她脚下踩的是不是自己垫出来的方块：是的话，寻路失败时可以跳回地面重来。 */
    static boolean onScaffold(Run run, EntityMaid maid) {
        return run.scaffold.containsKey(maid.blockPosition().below());
    }

    /**
     * 平着搭路：目标与脚下齐平、中间又走不过去时，就地铺一格、走过去，再接着铺。
     *
     * <p>玩家在起床战争里就是这么搭的：方块放在脚下同一层，人始终走在同一个高度。
     * 一直往上垫解决不了齐平的缺口——每垫一格就高一层，到头来还是够不着也看不见。
     * 前面一格本来就站得住（自然地面或刚铺好的路）就直接走过去，不多放一块。
     */
    static boolean bridgeToward(Run run, EntityMaid maid, Cell cell) {
        BlockPos feet = maid.blockPosition();
        int dx = cell.pos.getX() - feet.getX();
        int dz = cell.pos.getZ() - feet.getZ();
        if (!ScaffoldRules.mayBridge(cell.pos.getY() - feet.getY())
                || Math.abs(dx) + Math.abs(dz) < 2) {
            return false;
        }
        if (cell.bridges >= ScaffoldRules.MAX_BRIDGE
                || !ScaffoldRules.nearEnough(dx, dz)
                || run.scaffold.size() >= ScaffoldRules.MAX_BLOCKS
                || !canJumpHere(run, maid)) {
            return false;
        }
        Direction step = ScaffoldRules.alongX(dx, dz)
                ? Direction.getNearest(dx, 0, 0)
                : Direction.getNearest(0, 0, dz);
        BlockPos ahead = feet.relative(step);
        // 前面是目标格或图纸要的位置就不能站进去：那是要放方块的地方
        if (ahead.equals(cell.pos) || run.blueprint.contains(ahead)) {
            return false;
        }
        // 同一个落脚点走不过去一次就够了，不必每一遍都从原地再试一遍
        if (cell.triedStands.contains(ahead)) {
            return false;
        }
        if (freeForStep(run, ahead)
                && freeForStep(run, ahead.above())
                && !selfInside(ahead, maid)
                && !otherEntityInside(run, ahead, maid)) {
            if (solidCell(run.level, ahead.below())) {
                walkOnly(run, maid, cell, ahead);
                return true;
            }
            // 前面悬空：在它下面铺一块，走过去还是同一个高度
            BlockPos floor = ahead.below();
            if (run.blueprint.contains(floor)
                    || !freeForStep(run, floor)
                    || !withinReach(maid, floor)
                    || otherEntityInside(run, floor, maid)) {
                return false;
            }
            Item item = scaffoldItem(run, maid);
            if (item == null || !put(run, maid, cell, floor, item)) {
                return false;
            }
            cell.bridges++;
            run.cooldown = PASS_BREAK_TICKS;
            LOGGER.info("施工搭路：为 {} 把 {} 铺在 {}", cell.pos, item, floor);
            walkOnly(run, maid, cell, ahead);
            return true;
        }
        return false;
    }

    /**
     * 够不着时在她脚下垫一格：升上去、或者平着挪到能放的位置。
     *
     * <p>她的寻路只能上一格（再多就要跳跃输入），所以每 tick 只垫一格：先在她脚下
     * 这一层找一格空位，把方块放进去，再把寻路目标设到新方块的顶上。下一 tick 她
     * 站高一格，再判还差多少。目标还高过两层时，每垫一格都必须比现在更接近目标；
     * 目标与她齐平或只高一格时（屋顶那一步大多是这种），只要垫完放得下就垫，
     * 方向可以偏离目标——边上往往只有那一侧是空的。
     *
     * <p>托底那块先垫：顶块立在一块空地上放不住，得有下面的方块给它当依托。
     * 四面没有能垫的位置时改原地起跳，见 {@link #jumpUp}。
     */
    static boolean stepUp(Run run, EntityMaid maid, Cell cell) {
        if (cell.steps >= ScaffoldRules.MAX_STEPS
                || run.scaffold.size() >= ScaffoldRules.MAX_BLOCKS) {
            return false;
        }
        BlockPos feet = maid.blockPosition();
        int rise = cell.pos.getY() - feet.getY();
        if (!ScaffoldRules.mayStep(rise)) {
            return false;
        }
        // 头顶被方块压住时她既跳不起来也迈不上去：先走开（交给 stageOutside），别在原地耗
        if (!canJumpHere(run, maid)) {
            return false;
        }
        if (!ScaffoldRules.nearEnough(
                cell.pos.getX() - feet.getX(), cell.pos.getZ() - feet.getZ())) {
            return false;
        }
        Item item = scaffoldItem(run, maid);
        if (item == null) {
            return false;
        }
        Vec3 target = Vec3.atCenterOf(cell.pos);
        double current = Vec3.atBottomCenterOf(feet).distanceToSqr(target);
        // 目标就在正上方：原地跳起来把脚下垫高，别为了垫一格先横着挪出去再爬回来
        if (rise >= 2 && cell.pos.getX() == feet.getX() && cell.pos.getZ() == feet.getZ()
                && jumpUp(run, maid, cell)) {
            return true;
        }
        BlockPos bestTop = null;
        boolean bestSupportReady = false;
        double bestCost = Double.MAX_VALUE;
        BlockPos placeTop = null;
        boolean placeSupportReady = false;
        double placeCost = Double.MAX_VALUE;
        // 只往上下左右四个方向垫：斜着放她会从斜角起跳，被旁边的方块一挡就落回原高度
        for (Direction side : Direction.Plane.HORIZONTAL) {
            BlockPos top = feet.relative(side);
            if (!stepRoom(run, maid, top)) {
                continue;
            }
            double score = Vec3.atBottomCenterOf(top.above()).distanceToSqr(target);
            if (ScaffoldRules.mustApproach(rise) && score > current - 0.5) {
                continue;
            }
            BlockPos support = top.below();
            boolean supportReady = solidCell(run.level, support);
            if (!supportReady) {
                if (run.blueprint.contains(support) || !freeForStep(run, support)
                        || Placement.context(
                                        maid, new ItemStack(item), support, null, null, null)
                                == null
                        || !Placement.seesNow(maid, support, null)) {
                    continue;
                }
            } else if (!Placement.seesNow(maid, top, null)) {
                // 垫脚块也得是她看得见的地方：看不见就换一侧
                continue;
            }
            // 底下已经实心就省下一块料，同分时优先
            double cost = score + (supportReady ? 0.0 : 0.5);
            if (canPlaceFrom(maid, top.above(), cell.pos) && cost < placeCost) {
                placeCost = cost;
                placeTop = top;
                placeSupportReady = supportReady;
            }
            if (cost < bestCost) {
                bestCost = cost;
                bestTop = top;
                bestSupportReady = supportReady;
            }
        }
        // 垫完就能放的站位优先：目标与她齐平时，这一格就是为了站过去
        if (placeTop != null) {
            bestTop = placeTop;
            bestSupportReady = placeSupportReady;
        }
        if (bestTop == null) {
            // 四个方向都垫不出去（贴着墙、被围住）：只能原地往上垫
            return jumpUp(run, maid, cell);
        }
        if (!bestSupportReady) {
            if (!put(run, maid, cell, bestTop.below(), item)) {
                return false;
            }
            // 托底只是前半个动作，升高数等到顶块放上去才算
            run.move = new StepMove(cell, bestTop, item);
            run.cooldown = PASS_BREAK_TICKS;
            return true;
        }
        if (!put(run, maid, cell, bestTop, item)) {
            return false;
        }
        cell.steps++;
        climbOnto(run, maid, cell, bestTop.above());
        return true;
    }

    /**
     * 原地跳起来把脚下的方块补上：站在一格空位里起跳，脚一离地就把方块放进原来那一格，
     * 落下来就升了一整格。
     *
     * <p>四个方向都垫不出去（贴着墙、被围住），或者目标就在正上方时只能这么往上，玩家
     * 搭柱子也是这个手法。起跳要有三格净空：脚一格、头一格、跳起来还要一格，否则头先撞上
     * 天花板，一格也升不上去。
     */
    static boolean jumpUp(Run run, EntityMaid maid, Cell cell) {
        if (cell.steps >= ScaffoldRules.MAX_STEPS
                || run.scaffold.size() >= ScaffoldRules.MAX_BLOCKS) {
            return false;
        }
        if (!maid.onGround()) {
            return false;
        }
        BlockPos feet = maid.blockPosition();
        if (run.blueprint.contains(feet)
                || !run.level.getBlockState(feet).canBeReplaced()
                || !freeForStep(run, feet.above())
                || !freeForStep(run, feet.above(2))
                || !solidCell(run.level, feet.below())
                || otherEntityInside(run, feet, maid)) {
            return false;
        }
        Item item = scaffoldItem(run, maid);
        if (item == null) {
            return false;
        }
        MaidActuator.stopWalking(maid);
        MaidActuator.lookAtPos(maid, cell.pos);
        maid.setDeltaMovement(0.0, JUMP_POWER, 0.0);
        // 人一离地，脚下这块就不在碰撞盒里了：先把她要落脚的那一格记成站位，
        // 收尾拆垫脚块时才会护着它，不会刚放上去就拆掉
        run.stand = feet.above().immutable();
        run.move = new JumpMove(cell, feet.immutable(), item);
        return true;
    }

    /**
     * 从她脚下找一条跑跳路线，落到能放下这一格的站位上。
     *
     * <p>只在走不过去之后试：先算落点够不够得着、看不看得见，再验一次跳过不过得去。
     * 路线上的每一跳都按「平地四格、抬一格三格、往下最多三格」算；她站在高处、目标
     * 在下面时再放开到十格，建筑模式摔落伤害已经免掉，从屋檐跳下去比垫一路便宜。
     */
    static boolean hopToPlace(Run run, EntityMaid maid, Cell cell) {
        if (run.move != null || !maid.onGround() || run.hopPlanned) {
            return false;
        }
        // 一条路线要逐格验视线，很贵：同一 tick 只算一条，同一步没算通隔两秒再试
        if (cell.hopUntil > run.level.getGameTime()) {
            return false;
        }
        run.hopPlanned = true;
        Predicate<BlockPos> canPlace = stand -> canPlaceFrom(maid, stand, cell.pos);
        int maxFall = ParkourRules.MAX_FALL;
        List<BlockPos> chain = MaidParkour.planToPlace(
                maid, cell.pos, cell.triedStands, canPlace, maxFall);
        if (chain.isEmpty() && maid.getY() > cell.pos.getY() + ParkourRules.MAX_FALL) {
            // 高处够不到下面的站位：建筑模式免摔伤，直接从屋檐跳下去，不必先垫一路
            maxFall = ParkourRules.DEEP_FALL;
            chain = MaidParkour.planToPlace(
                    maid, cell.pos, cell.triedStands, canPlace, maxFall);
        }
        if (chain.isEmpty()) {
            cell.hopUntil = run.level.getGameTime() + HOP_RETRY_TICKS;
            return false;
        }
        run.stand = null;
        MaidParkour.Runner runner = new MaidParkour.Runner(maid, chain, maxFall);
        if (runner.failed()) {
            cell.triedStands.addAll(chain);
            cell.hopUntil = run.level.getGameTime() + HOP_RETRY_TICKS;
            return false;
        }
        run.move = new HopMove(cell, runner);
        return true;
    }

    /** 站到这一格之后够得着、也看得见目标吗：垫这一脚、跳这一步到底有没有用。 */
    static boolean canPlaceFrom(EntityMaid maid, BlockPos stand, BlockPos target) {
        if (stand.equals(target)) {
            return false;
        }
        Vec3 eye = Vec3.atBottomCenterOf(stand).add(0.0, maid.getEyeHeight(), 0.0);
        if (new AABB(target).distanceToSqr(eye) > MineRules.REACH * MineRules.REACH) {
            return false;
        }
        return Placement.seesFrom(maid, stand, target, null);
    }

    /** 这一格能不能当垫脚石：位置空着、头顶有地方站、不在图纸里、她伸手够得着。 */
    static boolean stepRoom(Run run, EntityMaid maid, BlockPos top) {
        if (run.blueprint.contains(top) || !freeForStep(run, top)) {
            return false;
        }
        // 站上去要两格高：脚一格、头一格，否则会被房顶卡住
        if (!freeForStep(run, top.above()) || !freeForStep(run, top.above(2))) {
            return false;
        }
        return !selfInside(top, maid)
                && !otherEntityInside(run, top, maid)
                && !otherEntityInside(run, top.above(), maid)
                && withinReach(maid, top);
    }

    static boolean freeForStep(Run run, BlockPos pos) {
        BlockState state = run.level.getBlockState(pos);
        if (!state.isAir() && !state.canBeReplaced()) {
            return false;
        }
        return !(state.getBlock() instanceof LiquidBlock);
    }

    static void climbOnto(Run run, EntityMaid maid, Cell cell, BlockPos stand) {
        // 必须真的踩到新方块上：差一格就判到站，她会从斜角起跳，落在原来高度的方块上
        startWalk(run, maid, cell, stand);
    }

    /** 垫脚用的方块：优先她自己背着的整方块；创造模式她背包空着时借一块圆石。 */
    static Item scaffoldItem(Run run, EntityMaid maid) {
        Item carried = MaidInventory.scaffoldBlock(maid);
        if (carried != null) {
            return carried;
        }
        return run.consumeMaterials ? null : Blocks.COBBLESTONE.asItem();
    }

    /** 放一块垫脚方块：扣料、记账，她走开后由 cleanScaffold 拆掉。 */
    static boolean put(Run run, EntityMaid maid, Cell cell, BlockPos pos, Item item) {
        if (run.scaffold.size() >= ScaffoldRules.MAX_BLOCKS) {
            return false;
        }
        if (run.consumeMaterials && !MaidInventory.has(maid, item)) {
            return false;
        }
        if (!freeForStep(run, pos) || selfInside(pos, maid) || otherEntityInside(run, pos, maid)) {
            return false;
        }
        Block block = ((BlockItem) item).getBlock();
        MaidActuator.stopWalking(maid);
        lookToward(maid, pos);
        BlockPlaceContext context =
                Placement.visibleContext(maid, new ItemStack(item), pos, null, null, null);
        if (context == null) {
            return false;
        }
        ((BlockItem) item).place(context);
        if (run.level.getBlockState(pos).getBlock() != block) {
            return false;
        }
        upright(run.level, pos);
        if (run.consumeMaterials && MaidInventory.takeOne(maid, item)) {
            ItemReporter.markRemoval(BuiltInRegistries.ITEM.getKey(item).toString(), 1);
        }
        maid.swing(InteractionHand.MAIN_HAND);
        run.nextPlace = null;
        run.nextPlaceIn = 0;
        run.scaffold.put(pos.immutable(), block);
        run.scaffoldOwner.put(pos.immutable(), cell);
        run.stepPlaced++;
        return true;
    }

    /**
     * 她已经离开的垫脚块就拆掉：留一地毛刺不像话，生存模式下把方块还回她背包。
     *
     * <p>人还踩在上面、或者正走过去的垫脚块先留着，否则会拆掉自己脚下的台阶。
     */
    static void cleanScaffold(Run run, EntityMaid maid) {
        if (run.scaffold.isEmpty()) {
            return;
        }
        // 正走过去、正在垫下一块、正在起跳或跑跳时先不拆：拆了脚下的台阶她就上不去了
        if (!run.forceClean && run.move != null) {
            return;
        }
        AABB body = maid.getBoundingBox().inflate(0.1);
        var iterator = run.scaffold.entrySet().iterator();
        while (iterator.hasNext()) {
            Map.Entry<BlockPos, Block> entry = iterator.next();
            BlockPos pos = entry.getKey();
            BlockState state = run.level.getBlockState(pos);
            if (state.getBlock() != entry.getValue()) {
                run.scaffoldOwner.remove(pos);
                iterator.remove();
                continue;
            }
            if (!run.forceClean && body.intersects(new AABB(pos))) {
                continue;
            }
            if (!run.forceClean && run.stand != null && pos.distSqr(run.stand) <= 4) {
                continue;
            }
            // 为还没放好的格子搭的桥和依托先留着：她还没走到那儿，拆了她就再也走不过去
            Cell owner = run.scaffoldOwner.get(pos);
            if (!run.forceClean && owner != null && owner.state.open()) {
                continue;
            }
            run.level.destroyBlock(pos, false);
            if (run.consumeMaterials) {
                giveBack(run, maid, entry.getValue().asItem());
            }
            run.scaffoldOwner.remove(pos);
            iterator.remove();
        }
    }

    /** 把垫脚拆下来的方块塞回她背包；塞不下就掉在她脚下。 */
    static void giveBack(Run run, EntityMaid maid, Item item) {
        ItemStack stack = new ItemStack(item);
        int count = stack.getCount();
        CombinedInvWrapper backpack = maid.getAvailableBackpackInv();
        for (int slot = 0; slot < backpack.getSlots() && !stack.isEmpty(); slot++) {
            if (slot == MaidInventory.HEADWEAR_SLOT) {
                continue;
            }
            stack = backpack.insertItem(slot, stack, false);
        }
        int inserted = count - stack.getCount();
        if (inserted > 0) {
            // 这块本来就该在她身上：背包差分别把它说成主人刚塞进来的
            ItemReporter.suppressHandGain(new ItemStack(item, inserted));
        }
        if (!stack.isEmpty()) {
            Block.popResource(run.level, maid.blockPosition(), stack);
        }
    }

}
