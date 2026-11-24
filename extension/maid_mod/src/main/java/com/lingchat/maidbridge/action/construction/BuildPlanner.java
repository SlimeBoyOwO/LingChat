package com.lingchat.maidbridge.action.construction;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.construction.BuildRules.Failure;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.action.mine.MaidMineController;
import com.lingchat.maidbridge.action.mine.MineTargets;
import com.lingchat.maidbridge.action.nav.Bodies;
import com.lingchat.maidbridge.action.nav.NavGoal;
import com.lingchat.maidbridge.action.nav.NavPolicy;
import com.lingchat.maidbridge.action.nav.Navigator;
import com.lingchat.maidbridge.action.nav.ScaffoldBook;
import com.lingchat.maidbridge.action.nav.Terrain;
import com.lingchat.maidbridge.action.place.Placement;
import com.lingchat.maidbridge.maid.MaidInventory;
import com.lingchat.maidbridge.perception.item.ItemReporter;
import com.lingchat.maidbridge.perception.spatial.TerrainMemory;
import java.util.function.Predicate;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.BlockStateProperties;
import net.minecraft.world.phys.Vec3;

import static com.lingchat.maidbridge.action.construction.BuildLayer.*;
import com.lingchat.maidbridge.action.construction.BuildRules.*;
import com.lingchat.maidbridge.action.construction.MaidBuildController.Cell;
import com.lingchat.maidbridge.action.construction.MaidBuildController.Run;
import static com.lingchat.maidbridge.action.construction.MaidBuildController.*;

/** 选格与落子：判定这一格是什么，再决定就地放、叫挖掘、还是让寻路把她送过去。 */
final class BuildPlanner {
    /**
     * 处理一格的结果：动手了、这一格先跳过，还是要接着走放置链。
     *
     * <p>CONTINUE 只在本地判定与放置链之间传：表示世界这边没问题，接着往下放置。
     * 对外的入口（work）永远不会把它交给调用方。
     */
    enum Step {
        HANDLED,
        SKIP,
        WAIT,
        CONTINUE
    }

    /**
     * 下一块大概率放哪：手上那一格优先，其次按「此刻够得着又看得见」从头找。
     */
    static BlockPos nextPlacement(Run run, EntityMaid maid) {
        if (run.active != null && run.active.state.open()) {
            return run.active.pos;
        }
        boolean doorsOnly = onlyDoorsLeft(run);
        int scanned = 0;
        int checked = 0;
        for (int i = run.layerStart; i < run.layerEnd && scanned < 64 && checked < 8; i++) {
            scanned++;
            Cell cell = run.cells.get(i);
            if (!inPass(cell, doorsOnly) || held(cell, run.level) || reservedStand(run, cell)) {
                continue;
            }
            // 和真正选格同一套判据：够得着又看得见才算「下一块」
            if (BuildRules.verdict(observe(run, maid, cell)) != BuildRules.Verdict.CONTINUE
                    || !withinReach(maid, cell.pos)) {
                continue;
            }
            checked++;
            if (visibleContext(maid, cell) != null) {
                return cell.pos;
            }
        }
        return null;
    }

    /**
     * 从当前游标往后找一格现在能处理的；处理了（放了或开始走过去）返回真。
     *
     * <p>手上那一格没做完之前先回头做它：走到一半没走通、到站了却看不见，都换站位重试，
     * 而不是直接扫下一格。
     */
    static boolean next(Run run, EntityMaid maid) {
        // 先把站在原地够得着的格子放掉，别为了一格远处的跑过去又跑回来
        if (placeReachable(run, maid)) {
            stop("原地放");
            return true;
        }
        Cell active = run.active;
        if (active != null && !active.state.open()) {
            run.active = null;
            active = null;
        }
        if (active != null) {
            Step step = workLimited(run, maid, active);
            if (step != Step.SKIP) {
                stop("手上做");
                return true;
            }
            run.active = null;
        }
        int scanned = 0;
        boolean doorsOnly = onlyDoorsLeft(run);
        while (run.cursor < run.layerEnd && scanned < MAX_SCAN_PER_TICK) {
            scanned++;
            Cell cell = run.cells.get(run.cursor++);
            // 门留到别的格子都放好之后再装，施工期间那个门洞一直敞着，她好进出
            if (!inPass(cell, doorsOnly) || held(cell, run.level)
                    || reservedStand(run, cell)) {
                continue;
            }
            run.active = cell;
            Step step = workLimited(run, maid, cell);
            if (step != Step.SKIP) {
                stop("扫到做");
                return true;
            }
            run.active = null;
        }
        return false;
    }

    /**
     * 处理一格，但给「整遍都卡在同一格」设个上限。
     *
     * <p>连续等这么久还找不到站位，就按这一遍没辙跳过它，下个 tick 从游标后面接着扫。
     */
    static Step workLimited(Run run, EntityMaid maid, Cell cell) {
        Step step = work(run, maid, cell);
        if (step == Step.WAIT) {
            if (++cell.waits > MAX_CELL_WAITS) {
                cell.waits = 0;
                CellFlow.holdThenPark(run, cell, Failure.NO_STAND);
                return Step.SKIP;
            }
        } else {
            cell.waits = 0;
        }
        return step;
    }

    /**
     * 调用一格的场景：决定这一格能不能发起新的移动，以及同一种失败怎么处置。
     */
    private enum Mode {
        /** 原地快放：只处理已有定论的格子和够得着的落子，其余留给扫描。 */
        PROBE,
        /** 正常推进：可以叫寻路。 */
        SCAN,
        /** 刚走到落脚点：只判定与落子，不再发起新的移动。 */
        ARRIVE
    }

    /** 处理一格的完整流程：先按本地情况判定，再决定挖掉、放置还是叫寻路。 */
    static Step work(Run run, EntityMaid maid, Cell cell) {
        if (reservedStand(run, cell)) {
            return Step.SKIP;
        }
        Step step = handleVerdict(run, maid, cell, Mode.SCAN);
        return step == Step.CONTINUE ? placeChain(run, maid, cell) : step;
    }

    /**
     * 本地判定：这一格此刻是什么、该怎么处置。
     *
     * <p>返回 CONTINUE 表示世界这边没问题，交给后面的放置链。
     */
    private static Step handleVerdict(Run run, EntityMaid maid, Cell cell, Mode mode) {
        return switch (BuildRules.verdict(observe(run, maid, cell))) {
            case DONE -> {
                CellFlow.done(run, cell);
                yield Step.SKIP;
            }
            case LIQUID -> {
                CellFlow.fail(run, cell, Failure.LIQUID);
                yield Step.SKIP;
            }
            case MISSING -> {
                CellFlow.fail(run, cell, Failure.SHORT_OF_MATERIAL);
                yield Step.SKIP;
            }
            case HELD_BY_ENTITY -> {
                if (mode == Mode.PROBE) {
                    yield Step.SKIP;
                }
                // 别的活物挡着：先不管，过一会儿自动重试
                cell.holdUntil = run.level.getGameTime() + ENTITY_RETRY_TICKS;
                CellFlow.hold(run, cell, Failure.BLOCKED_BY_ENTITY);
                yield Step.SKIP;
            }
            case DIG_BLOCKED -> {
                if (mode == Mode.PROBE) {
                    yield Step.SKIP;
                }
                if (mode == Mode.SCAN) {
                    run.blockedSkips++;
                    CellFlow.park(run, cell, Failure.OCCUPIED);
                } else {
                    CellFlow.hold(run, cell, Failure.OCCUPIED);
                }
                yield Step.SKIP;
            }
            case DIG -> {
                if (mode == Mode.PROBE) {
                    yield Step.SKIP;
                }
                // 被别的方块占着：够得着就挖掉（掉落进她背包），够不着交给寻路送过去
                if (MineTargets.canMine(maid, cell.pos)) {
                    clearCell(run, maid, cell);
                    yield Step.HANDLED;
                }
                if (mode == Mode.ARRIVE) {
                    CellFlow.hold(run, cell, Failure.OCCUPIED);
                    yield Step.SKIP;
                }
                startNav(run, maid, cell, stand -> canDigFrom(maid, stand, cell.pos));
                yield Step.HANDLED;
            }
            case SELF_INSIDE -> {
                if (mode == Mode.PROBE) {
                    yield Step.SKIP;
                }
                if (mode == Mode.ARRIVE) {
                    // 到了还压着这一格：把这个站位记下来，免得下轮又走回来
                    rememberStand(run, cell);
                    CellFlow.holdThenPark(run, cell, Failure.SELF_BLOCKED);
                    if (cell.state.open()) {
                        run.active = cell;
                    }
                    yield Step.SKIP;
                }
                // 自己站在要放的那一格：让寻路把她挪出去，必要时在旁边垫一格站
                startNav(run, maid, cell, placeStand(maid, cell.pos));
                yield Step.HANDLED;
            }
            case CONTINUE -> Step.CONTINUE;
        };
    }

    /**
     * 世界这边没问题：够得着又看得见就放，否则把「站到哪里」交给寻路。
     */
    private static Step placeChain(Run run, EntityMaid maid, Cell cell) {
        if (context(maid, cell) == null) {
            // 依托面还没出现：先垫一块依托；够不着就请寻路把她送到够得着的位置
            if (prop(run, maid, cell)) {
                return Step.HANDLED;
            }
            if (!withinReach(maid, cell.pos) && goPlace(run, maid, cell)) {
                return Step.HANDLED;
            }
            run.noSupportSkips++;
            return Step.SKIP;
        }
        if (withinReach(maid, cell.pos) && visibleContext(maid, cell) != null) {
            place(run, maid, cell);
            return Step.HANDLED;
        }
        if (goPlace(run, maid, cell)) {
            return Step.HANDLED;
        }
        CellFlow.holdThenPark(run, cell,
                canJumpHere(maid) ? Failure.NO_STAND : Failure.NO_HEADROOM);
        return Step.SKIP;
    }

    /** 叫寻路站到能放下这一格的位置。 */
    static boolean goPlace(Run run, EntityMaid maid, Cell cell) {
        startNav(run, maid, cell, placeStand(maid, cell.pos));
        return true;
    }

    /**
     * 能站过去放下这一格的站位。
     *
     * <p>这里不能排除她当前站的这一格：寻路每 tick 先用同一个判据问「到了没」，排除掉就永远
     * 到不了，只能等 600 tick 超时。当前这格真放不下时，{@link #arrive} 会把它记进试过的
     * 站位，下一次搜索自然跳过。
     */
    private static Predicate<BlockPos> placeStand(EntityMaid maid, BlockPos target) {
        return stand -> canActFrom(maid, stand, target);
    }

    /** 给她开一次寻路：目标、边界、记忆都从这一格上拿。 */
    static void startNav(Run run, EntityMaid maid, Cell cell, Predicate<BlockPos> canAct) {
        NavPolicy policy = new NavPolicy(run.blueprint, run.built, run.consumeMaterials);
        run.nav = Navigator.start(
                new NavGoal.Stand(cell.pos, canAct), policy, cell.nav, run.book);
        run.navCell = cell;
        run.active = cell;
        run.standFor = cell;
        run.stand = null;
        run.cooldown = 0;
    }

    /** 站到这一格之后能放下目标吗：够得着、看得见，身体也不能压着目标格。 */
    static boolean canActFrom(EntityMaid maid, BlockPos stand, BlockPos target) {
        if (stand.equals(target)) {
            return false;
        }
        // 已经站在这一格时量真实碰撞箱；还没过去时按站到格心的身体估算
        boolean inside = stand.equals(maid.blockPosition())
                ? Bodies.self(target, maid)
                : Bodies.selfAt(stand, target, maid);
        if (inside) {
            return false;
        }
        return Terrain.reachableFrom(maid, stand, target)
                && Placement.seesFrom(maid, stand, target, null);
    }

    /** 站到这一格之后能挖到目标吗。 */
    static boolean canDigFrom(EntityMaid maid, BlockPos stand, BlockPos target) {
        if (stand.equals(target)) {
            return false;
        }
        return Terrain.reachableFrom(maid, stand, target)
                && Placement.seesFrom(maid, stand, target, null);
    }

    /**
     * 到站之后的收尾：只看这一格还能不能放，放不了就记下原因并留着它。
     *
     * <p>不发起新的移动——跑跳、垫脚、搭路留给下个 tick 的扫描决定。
     */
    static void arrive(Run run, EntityMaid maid) {
        Cell cell = run.active;
        if (cell == null || !cell.state.open()) {
            return;
        }
        Step step = handleVerdict(run, maid, cell, Mode.ARRIVE);
        if (step != Step.CONTINUE) {
            run.cooldown = PASS_BREAK_TICKS;
            return;
        }
        if (context(maid, cell) == null) {
            // 依托面还没出现：下一遍再试，不记失败次数
            run.cooldown = PASS_BREAK_TICKS;
            return;
        }
        if (!withinReach(maid, cell.pos)) {
            // 垫上去之后可能还差一点：别在超出玩家距离的地方把方块放出来
            rememberStand(run, cell);
            CellFlow.hold(run, cell, Failure.OUT_OF_REACH);
            run.cooldown = PASS_BREAK_TICKS;
            return;
        }
        if (visibleContext(maid, cell) == null) {
            rememberStand(run, cell);
            CellFlow.hold(run, cell, Failure.NO_SIGHT);
            run.cooldown = PASS_BREAK_TICKS;
            return;
        }
        place(run, maid, cell);
    }

    /** 记下这次没放成的站位，下一轮换位置时跳过，避免来回走。 */
    private static void rememberStand(Run run, Cell cell) {
        if (run.stand != null) {
            cell.nav.triedStands.add(run.stand);
        }
    }

    /** 寻路失败时把原因翻成格子账上的原因。 */
    static Failure failureOf(com.lingchat.maidbridge.action.nav.NavFailure nav) {
        if (nav == null) {
            return Failure.NO_PATH;
        }
        return switch (nav) {
            case NO_STAND -> Failure.NO_STAND;
            case NO_PATH -> Failure.NO_PATH;
            case NO_HEADROOM -> Failure.NO_HEADROOM;
            case OUT_OF_REACH -> Failure.OUT_OF_REACH;
            case TIMEOUT -> Failure.NO_PATH;
        };
    }

    /**
     * 这一格是不是她正赶过去的落脚点：到站之前放上去，她就没地方站了。
     */
    static boolean reservedStand(Run run, Cell cell) {
        Cell owner = run.standFor;
        return run.stand != null && owner != null && owner != cell
                && owner.state.open() && cell.pos.equals(run.stand);
    }

    /** 这一格放的是不是图纸要的那一块：火把挂到墙上时原版会换成 wall_torch。 */
    static boolean matches(Cell cell, BlockState state) {
        Block block = state.getBlock();
        return cell.block == block || (cell.block == Blocks.TORCH && block == Blocks.WALL_TORCH);
    }

    /** 收集一格的本地情况，交给纯规则判定；不做站位搜索和视线判定。 */
    static BuildRules.Situation observe(Run run, EntityMaid maid, Cell cell) {
        BlockState state = run.level.getBlockState(cell.pos);
        boolean occupied = !state.isAir() && !state.canBeReplaced();
        boolean usable = !occupied && !(state.getBlock() instanceof LiquidBlock);
        return new BuildRules.Situation(
                matches(cell, state),
                state.getBlock() instanceof LiquidBlock,
                occupied,
                occupied && maid.canDestroyBlock(cell.pos),
                occupied || !run.consumeMaterials || MaidInventory.count(maid, cell.item) > 0,
                usable && otherEntityInside(run, cell.pos, maid),
                usable && selfInside(cell.pos, maid));
    }

    /** 站在原地伸手够得着的格子先放掉；一格都放不了返回假。 */
    static boolean placeReachable(Run run, EntityMaid maid) {
        boolean doorsOnly = onlyDoorsLeft(run);
        for (int i = run.layerStart; i < run.layerEnd; i++) {
            Cell cell = run.cells.get(i);
            if (!inPass(cell, doorsOnly) || held(cell, run.level) || reservedStand(run, cell)) {
                continue;
            }
            if (handleVerdict(run, maid, cell, Mode.PROBE) != Step.CONTINUE) {
                continue;
            }
            // 够得着、看得见就放；其余的留给 next() 去叫寻路
            if (withinReach(maid, cell.pos) && visibleContext(maid, cell) != null) {
                run.active = cell;
                place(run, maid, cell);
                return true;
            }
        }
        return false;
    }

    /**
     * 目标这一格四面都没有能贴的面：在它正下方垫一块实心方块当依托，下一遍再放目标。
     *
     * <p>最常见的是图纸把格子落在草、雪、高草这类可替换的方块上：原版点下去会把这些
     * 方块本身换掉，拼出来的落点不是目标格。先把脚下换成一块实心的垫脚方块，
     * 目标就有面可以贴了；收尾拆垫脚块时它会一起收走。
     */
    static boolean prop(Run run, EntityMaid maid, Cell cell) {
        if (cell.propped) {
            return false;
        }
        BlockPos foot = cell.pos.below();
        if (run.blueprint.contains(foot)
                || Terrain.solid(run.level, foot)
                || !Terrain.free(run.level, foot)
                || !withinReach(maid, foot)) {
            return false;
        }
        Item item = ScaffoldBook.item(maid, run.consumeMaterials);
        // 先核几何：贴不上的位置不必转头去试，免得每扫到这一格就把她的视线拽过来
        if (item == null
                || Placement.context(maid, new ItemStack(item), foot, null, null, null) == null
                || !run.book.put(run.level, maid, foot, item, run.consumeMaterials)) {
            return false;
        }
        cell.propped = true;
        run.cooldown = PASS_BREAK_TICKS;
        LOGGER.info("施工垫依托：为 {} 把 {} 垫在 {}", cell.pos, item, foot);
        return true;
    }

    static void place(Run run, EntityMaid maid, Cell cell) {
        MaidActuator.stopWalking(maid);
        lookToward(maid, cell.pos);
        BlockPlaceContext context = visibleContext(maid, cell);
        if (context == null) {
            CellFlow.hold(run, cell, Failure.NO_SIGHT);
            run.active = cell;
            return;
        }
        ((BlockItem) cell.item).place(context);
        BlockState after = run.level.getBlockState(cell.pos);
        if (!matches(cell, after)) {
            CellFlow.hold(run, cell, Failure.PLACE_FAILED);
            return;
        }
        upright(run.level, cell.pos);
        if (run.consumeMaterials && MaidInventory.takeOne(maid, cell.item)) {
            // 少掉的是她放下去的那一块，账上记一笔：不然背包差分把这次消耗说成主人拿走了
            ItemReporter.markRemoval(cell.itemId, 1);
        }
        maid.swing(InteractionHand.MAIN_HAND);
        run.active = null;
        run.nextPlace = null;
        run.nextPlaceIn = 0;
        CellFlow.done(run, cell);
        run.cooldown = placeInterval(run, maid, cell);
    }

    /**
     * 把目标格里的别的方块挖掉：工具、耐久、掉落都按挖掘那一套来。
     */
    static void clearCell(Run run, EntityMaid maid, Cell cell) {
        BlockState state = run.level.getBlockState(cell.pos);
        if (!maid.canDestroyBlock(cell.pos)) {
            CellFlow.park(run, cell, Failure.OCCUPIED);
            return;
        }
        MineTargets.equipBestTool(maid, state);
        MaidActuator.stopWalking(maid);
        MaidActuator.lookAtPos(maid, cell.pos);
        if (!maid.destroyBlock(cell.pos, true)) {
            CellFlow.park(run, cell, Failure.OCCUPIED);
            return;
        }
        maid.swing(InteractionHand.MAIN_HAND);
        ItemStack used = maid.getMainHandItem();
        if (!used.isEmpty()) {
            used.hurtAndBreak(1, maid, EquipmentSlot.MAINHAND);
        }
        TerrainMemory.reveal(maid, cell.pos, run.level.getGameTime());
        MaidMineController.markMined(maid);
    }

    /** 原木这类有朝向轴的方块强制竖着放，不按她点的那一面横过来。 */
    static void upright(ServerLevel level, BlockPos pos) {
        BlockState state = level.getBlockState(pos);
        if (!state.hasProperty(BlockStateProperties.AXIS)
                || state.getValue(BlockStateProperties.AXIS) == Direction.Axis.Y) {
            return;
        }
        level.setBlock(pos, state.setValue(BlockStateProperties.AXIS, Direction.Axis.Y), 3);
    }

    /**
     * 每块之间留一点随机间隔；下一格要转的角度越大，多等几 tick 让视线先转过去。
     */
    static int placeInterval(Run run, EntityMaid maid, Cell placed) {
        int interval = PLACE_INTERVAL_BASE + run.level.random.nextInt(PLACE_INTERVAL_JITTER);
        BlockPos next = predictNext(run, maid);
        if (next == null || next.equals(placed.pos)) {
            return interval;
        }
        // 先把视线挪到下一格，冷却这段时间她正好转头；不然头还没动方块就出来了
        MaidActuator.lookAtPos(maid, next);
        Vec3 eye = maid.getEyePosition();
        Vec3 from = Vec3.atCenterOf(placed.pos).subtract(eye);
        Vec3 to = Vec3.atCenterOf(next).subtract(eye);
        if (from.lengthSqr() < 1.0E-4 || to.lengthSqr() < 1.0E-4) {
            return interval;
        }
        double cosine = from.normalize().dot(to.normalize());
        double angle = Math.acos(Math.max(-1.0, Math.min(1.0, cosine)));
        int extra = (int) Math.round(angle / (Math.PI / 6));
        return interval + Math.min(TURN_EXTRA_TICKS, extra);
    }

    /**
     * 把身子和头一起转向这一格再动手。
     *
     * <p>转头控制一 tick 只转三十度，放方块却就在同一 tick：只写视线目标的话，方块
     * 常常是她背对着放下的，朝向也跟着旧身体角度走。
     */
    static void lookToward(EntityMaid maid, BlockPos pos) {
        Vec3 eye = maid.getEyePosition();
        double dx = pos.getX() + 0.5 - eye.x;
        double dy = pos.getY() + 0.5 - eye.y;
        double dz = pos.getZ() + 0.5 - eye.z;
        float yaw = (float) (Math.toDegrees(Math.atan2(dz, dx)) - 90.0);
        float pitch = (float) -Math.toDegrees(Math.atan2(dy, Math.sqrt(dx * dx + dz * dz)));
        maid.setYRot(yaw);
        maid.setYHeadRot(yaw);
        maid.setYBodyRot(yaw);
        maid.setXRot(pitch);
        MaidActuator.lookAtPos(maid, pos);
    }

    static BlockPlaceContext context(EntityMaid maid, Cell cell) {
        return Placement.context(maid, new ItemStack(cell.item), cell.pos, null, null, cell.yaw);
    }

    static BlockPlaceContext visibleContext(EntityMaid maid, Cell cell) {
        return Placement.visibleContext(
                maid, new ItemStack(cell.item), cell.pos, null, null, cell.yaw);
    }

    /** 手够不够得着：按原版口径量眼睛到方块外壳的距离。 */
    static boolean withinReach(EntityMaid maid, BlockPos target) {
        return Terrain.reachable(maid, target);
    }

    static boolean selfInside(BlockPos target, EntityMaid maid) {
        return Bodies.self(target, maid);
    }

    static boolean canJumpHere(EntityMaid maid) {
        BlockPos feet = maid.blockPosition();
        return Terrain.free(maid.level(), feet.above())
                && Terrain.free(maid.level(), feet.above(2));
    }

    /** 这一格里有没有挡着放置的活物。 */
    static boolean otherEntityInside(Run run, BlockPos target, EntityMaid maid) {
        return Bodies.other(run.level, target, maid);
    }
}
