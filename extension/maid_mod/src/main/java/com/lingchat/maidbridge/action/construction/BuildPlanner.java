package com.lingchat.maidbridge.action.construction;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.construction.BuildRules.Failure;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.action.mine.MaidMineController;
import com.lingchat.maidbridge.action.mine.MineRules;
import com.lingchat.maidbridge.action.mine.MineTargets;
import com.lingchat.maidbridge.action.place.Placement;
import com.lingchat.maidbridge.maid.MaidInventory;
import com.lingchat.maidbridge.perception.item.ItemReporter;
import com.lingchat.maidbridge.perception.spatial.TerrainMemory;
import java.util.function.Predicate;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.BlockStateProperties;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;

import static com.lingchat.maidbridge.action.construction.BuildLayer.*;
import static com.lingchat.maidbridge.action.construction.BuildMoves.*;
import com.lingchat.maidbridge.action.construction.BuildRules.*;
import com.lingchat.maidbridge.action.construction.MaidBuildController.Cell;
import com.lingchat.maidbridge.action.construction.MaidBuildController.Run;
import static com.lingchat.maidbridge.action.construction.MaidBuildController.*;

/** 选格与落子：判定、走路、挖掉、放置。 */
final class BuildPlanner {
    /** 处理一格的结果：动手了、这一格先跳过、还是这一 tick 的寻路额度用完了。 */
    enum Step {
        HANDLED,
        SKIP,
        WAIT
    }

    /**
     * 下一块大概率放哪：手上正垫的顶块/起跳位优先，其次是要走过去的那一格；
     * 都放过之后按「此刻够得着又看得见」从头找，跟真正的放置顺序同一套判据。
     */
    static BlockPos nextPlacement(Run run, EntityMaid maid) {
        if (run.move != null) {
            return run.move.nextPlace();
        }
        if (run.cleaning) {
            return null;
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
     * 而不是直接扫下一格。否则一层两头各有一格放着，她就会在这条线上来回跑。
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
                stop(step == Step.WAIT ? "手上等额度" : "手上做");
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
                stop(step == Step.WAIT ? "扫到等额度" : "扫到做");
                return true;
            }
            run.active = null;
        }
        return false;
    }

    /**
     * 处理一格，但给「寻路额度用完」设个上限。
     *
     * <p>额度每 tick 都会补满，可只要这一格每次都能把额度吃光又找不到站位，它就会整遍
     * 都停在 WAIT 上：后面的格子轮不到，这一层也永远走不完。连续等这么久就按这一遍
     * 没辙跳过它，下个 tick 从游标后面接着扫。
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
     * 处理一格的完整流程：先按本地情况判定，再决定挖掉、走路还是放置。
     *
     * <p>WAIT 表示寻路额度用完了：这一格留着（{@link Run#active}）下个 tick 接着来，
     * 不能被当成「扫完了」把游标推过去。
     */
    static Step work(Run run, EntityMaid maid, Cell cell) {
        if (reservedStand(run, cell)) {
            return Step.SKIP;
        }
        switch (BuildRules.verdict(observe(run, maid, cell))) {
            case DONE -> {
                CellFlow.done(run, cell);
                return Step.SKIP;
            }
            case LIQUID -> {
                CellFlow.fail(run, cell, Failure.LIQUID);
                return Step.SKIP;
            }
            case DIG_BLOCKED -> {
                run.blockedSkips++;
                CellFlow.park(run, cell, Failure.OCCUPIED);
                return Step.SKIP;
            }
            case DIG -> {
                // 被别的方块占着：够得着就挖掉（掉落进她背包），够不着先走过去
                if (MineTargets.canMine(maid, cell.pos)) {
                    clearCell(run, maid, cell);
                    return Step.HANDLED;
                }
                BlockPos dig = MineTargets.standFor(maid, cell.pos);
                if (dig == null) {
                    CellFlow.hold(run, cell, Failure.NO_STAND);
                    return Step.SKIP;
                }
                startWalk(run, maid, cell, dig);
                return Step.HANDLED;
            }
            case MISSING -> {
                CellFlow.fail(run, cell, Failure.SHORT_OF_MATERIAL);
                return Step.SKIP;
            }
            case HELD_BY_ENTITY -> {
                // 别的活物挡着：先不管，过一会儿自动重试
                cell.holdUntil = run.level.getGameTime() + ENTITY_RETRY_TICKS;
                CellFlow.hold(run, cell, Failure.BLOCKED_BY_ENTITY);
                return Step.SKIP;
            }
            case SELF_INSIDE -> {
                // 自己站在要放的那一格：先挪出去，别把自己埋了
                BlockPos away = MineTargets.standForBuilding(
                        maid, cell.pos, run.built, cell.triedStands, run.unreachable,
                        stand -> true, run.search);
                if (away != null && !away.equals(maid.blockPosition())) {
                    startWalk(run, maid, cell, away);
                    return Step.HANDLED;
                }
                // 周围没有实地可站（屋顶、墙头边上就是空的）：在旁边空中搭一格再站过去
                if (stepAside(run, maid, cell)) {
                    return Step.HANDLED;
                }
                if (run.search.spent()) {
                    return Step.WAIT;
                }
                CellFlow.hold(run, cell, Failure.SELF_BLOCKED);
                return Step.SKIP;
            }
            case CONTINUE -> {
            }
        }
        // 目标明显在下面：先跳下去落到地面，再从地面找站位——她不会跳崖，
        // 站在屋顶上往墙根那几格放，怎么算都走不过去
        if (cell.pos.getY() <= maid.blockPosition().getY() - 2 && dropDown(run, maid, cell)) {
            return Step.HANDLED;
        }
        // 依托面还没出现（屋顶中间那格要等旁边先放）：先跳过，不算这一格失败，
        // 否则试满六次就被当成够不着，后面放好的邻居也救不回来
        if (context(maid, cell) == null) {
            // 四周一个实心邻居都没有时不干等：先把目标底下垫实，或者平着搭到它跟前
            if (prop(run, maid, cell)) {
                return Step.HANDLED;
            }
            if (bridgeToward(run, maid, cell)) {
                return Step.HANDLED;
            }
            run.noSupportSkips++;
            return Step.SKIP;
        }
        if (withinReach(maid, cell.pos)) {
            // 够得着还要看得见：隔着一面墙、一根柱子伸过去是不行的
            if (visibleContext(maid, cell) != null) {
                place(run, maid, cell);
                return Step.HANDLED;
            }
            if (walkToAnotherStand(run, maid, cell)) {
                return Step.HANDLED;
            }
            // 换不到站位：缺口另一头够得着、又站得住的话，先试试跑跳过去
            if (hopToPlace(run, maid, cell)) {
                return Step.HANDLED;
            }
            // 齐平的缺口先平着搭过去：往上垫只会离目标更远
            if (bridgeToward(run, maid, cell)) {
                return Step.HANDLED;
            }
            // 还是不行就再垫一格把眼睛抬高：柱子顶上那几格只能这么看见。
            // 垫脚不花寻路额度，必须先于「额度用完」的判断，否则选站位吃掉额度后
            // 她每 tick 都只等到一个 WAIT，永远走不到垫脚这一步
            if (stepUp(run, maid, cell)) {
                return Step.HANDLED;
            }
            if (run.search.spent()) {
                return Step.WAIT;
            }
            if (stageOutside(run, maid, cell)) {
                return Step.HANDLED;
            }
            CellFlow.holdThenPark(run, cell, canJumpHere(run, maid) ? Failure.NO_SIGHT : Failure.NO_HEADROOM);
            return Step.SKIP;
        }
        Predicate<BlockPos> canSee = stand -> Placement.seesFrom(maid, stand, cell.pos, null);
        BlockPos stand = MineTargets.standForBuilding(
                maid, cell.pos, run.built, cell.triedStands, run.unreachable, canSee,
                run.search);
        if (stand == null) {
            // 没有能走过去的站位：缺口另一头若一跳能到，就不用垫方块
            if (hopToPlace(run, maid, cell)) {
                return Step.HANDLED;
            }
            // 同一层的缺口：就地铺一格走过去，比往上垫更接近目标
            if (bridgeToward(run, maid, cell)) {
                return Step.HANDLED;
            }
            // 垫脚不花寻路额度：先垫一脚，不能因为选站位吃掉额度就停在这里等
            if (stepUp(run, maid, cell)) {
                return Step.HANDLED;
            }
            if (run.search.spent()) {
                return Step.WAIT;
            }
            if (stageOutside(run, maid, cell)) {
                return Step.HANDLED;
            }
            // 脚下踩的是自己垫的方块或桥，站着也找不到路：先跳回地面的实地，下一 tick
            // 从地面重新开始——在半空里反复算站位只会一直卡住
            if (onScaffold(run, maid) && dropDown(run, maid, cell)) {
                return Step.HANDLED;
            }
            CellFlow.holdThenPark(run, cell, canJumpHere(run, maid) ? Failure.NO_STAND : Failure.NO_HEADROOM);
            return Step.SKIP;
        }
        startWalk(run, maid, cell, stand);
        return Step.HANDLED;
    }

    /** 够得着却看不见这一格：换一个能看见它的站位再走过去。 */
    static boolean walkToAnotherStand(Run run, EntityMaid maid, Cell cell) {
        if (cell.triedStands.size() >= MAX_STAND_TRIES) {
            return false;
        }
        Predicate<BlockPos> canSee = stand -> Placement.seesFrom(maid, stand, cell.pos, null);
        BlockPos other = MineTargets.standForBuilding(
                maid, cell.pos, run.built, cell.triedStands, run.unreachable, canSee,
                run.search);
        if (other == null || other.equals(maid.blockPosition())) {
            return false;
        }
        startWalk(run, maid, cell, other);
        return true;
    }

    /**
     * 这一格是不是她正赶过去的落脚点：到站之前放上去，她就没地方站了。
     *
     * <p>只挡「为别的格子选的落脚点」：那一格自己的目标做完就放开，之后该放照样放。
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
            switch (BuildRules.verdict(observe(run, maid, cell))) {
                case DONE -> CellFlow.done(run, cell);
                case LIQUID -> CellFlow.fail(run, cell, Failure.LIQUID);
                case MISSING -> CellFlow.fail(run, cell, Failure.SHORT_OF_MATERIAL);
                case CONTINUE -> {
                    // 够得着、看得见就放；其余的留给 next() 去挖、走或垫
                    if (withinReach(maid, cell.pos) && visibleContext(maid, cell) != null) {
                        run.active = cell;
                        place(run, maid, cell);
                        return true;
                    }
                }
                default -> {
                    // 被占、被活物挡、自己占着：留给 next()
                }
            }
        }
        return false;
    }

    static void tryPlace(Run run, EntityMaid maid, Cell cell) {
        switch (BuildRules.verdict(observe(run, maid, cell))) {
            case DONE -> {
                CellFlow.done(run, cell);
                return;
            }
            case LIQUID -> {
                CellFlow.fail(run, cell, Failure.LIQUID);
                return;
            }
            case DIG_BLOCKED -> {
                CellFlow.hold(run, cell, Failure.OCCUPIED);
                return;
            }
            case DIG -> {
                // 走过来发现这一格被别的方块占着：能挖就挖掉，下一遍再放
                if (MineTargets.canMine(maid, cell.pos)) {
                    clearCell(run, maid, cell);
                } else {
                    CellFlow.hold(run, cell, Failure.OCCUPIED);
                }
                return;
            }
            case MISSING -> {
                CellFlow.fail(run, cell, Failure.SHORT_OF_MATERIAL);
                return;
            }
            case HELD_BY_ENTITY -> {
                // 别的活物挡着：先放一放，过一会儿自动重试
                cell.holdUntil = run.level.getGameTime() + ENTITY_RETRY_TICKS;
                CellFlow.hold(run, cell, Failure.BLOCKED_BY_ENTITY);
                return;
            }
            case SELF_INSIDE -> {
                // 还站在这一格里：下个 tick 先挪开再放
                run.active = cell;
                return;
            }
            case CONTINUE -> {
            }
        }
        if (!withinReach(maid, cell.pos)) {
            // 垫上去之后可能还差一点：别在超出玩家距离的地方把方块放出来，换个站位再试
            if (run.stand != null) {
                cell.triedStands.add(run.stand);
            }
            CellFlow.hold(run, cell, Failure.OUT_OF_REACH);
            run.active = cell;
            return;
        }
        if (visibleContext(maid, cell) == null) {
            // 依托面还没出现，或者目标被别的方块挡住：下一遍再试，不记失败次数
            if (context(maid, cell) == null) {
                return;
            }
            if (run.stand != null) {
                cell.triedStands.add(run.stand);
            }
            CellFlow.hold(run, cell, Failure.NO_SIGHT);
            // 那一格还留着：下个 tick 先换个看得见它的站位，别扫到别处去
            run.active = cell;
            return;
        }
        run.active = cell;
        place(run, maid, cell);
    }

    /**
     * 目标这一格四面都没有能贴的面：在它正下方垫一块实心方块当依托，下一遍再放目标。
     *
     * <p>最常见的是图纸把格子落在草、雪、高草这类可替换的方块上：原版点下去会把这些
     * 方块本身换掉，拼出来的落点不是目标格，于是永远「没有依托面」。先把脚下换成一块
     * 实心的垫脚方块，目标就有面可以贴了；收尾拆垫脚块时它会一起收走，目标悬着不碍事。
     */
    static boolean prop(Run run, EntityMaid maid, Cell cell) {
        if (cell.propped) {
            return false;
        }
        BlockPos foot = cell.pos.below();
        if (run.blueprint.contains(foot)
                || solidCell(run.level, foot)
                || !freeForStep(run, foot)
                || !withinReach(maid, foot)) {
            return false;
        }
        Item item = scaffoldItem(run, maid);
        // 先核几何：贴不上的位置不必转头去试，免得每扫到这一格就把她的视线拽过来
        if (item == null
                || Placement.context(maid, new ItemStack(item), foot, null, null, null) == null
                || !put(run, maid, cell, foot, item)) {
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
        // 世界变了，之前算不到的站位可能就能走到了
        run.unreachable.clear();
    }

    /**
     * 把目标格里的别的方块挖掉：工具、耐久、掉落都按挖掘那一套来。
     *
     * <p>车万女仆的 {@code destroyBlock} 会把掉落直接塞进她的库存；这一笔是她自己挖的，
     * 让背包差分别再报成「主人放进来的」。
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
        // 世界变了，之前算不到的站位可能就能走到了
        run.unreachable.clear();
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
     *
     * <p>固定间隔会让整栋楼像机器刷出来的一样；视线从墙这头跳到那头时，她的头还没
     * 转过去，下一块就已经放上了。
     */
    static int placeInterval(Run run, EntityMaid maid, Cell placed) {
        int interval = PLACE_INTERVAL_BASE + run.level.random.nextInt(PLACE_INTERVAL_JITTER);
        // 下一块大概率是那一格就看向那一格：以前取本层第一格，视线经常跟真正要放的对不上
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
     * 常常是她背对着放下的，朝向也跟着旧身体角度走。这里先把朝向拨到位再拼放置上下文。
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

    /**
     * 手够不够得着：按原版口径量眼睛到方块外壳的距离，不是到方块中心。
     *
     * <p>中心距离会把「站在旁边、眼睛到中心五格」也算够得着，实际却点不到面。
     */
    static boolean withinReach(EntityMaid maid, BlockPos target) {
        return new AABB(target).distanceToSqr(maid.getEyePosition())
                <= MineRules.REACH * MineRules.REACH;
    }

    static boolean selfInside(BlockPos target, EntityMaid maid) {
        return maid.getBoundingBox().intersects(new AABB(target));
    }

    /**
     * 这一格里有没有挡着放置的活物。
     *
     * <p>只认活着的生物：掉落物、箭这类实体不挡原版放置，物品也常常堆在工地里不动，
     * 把它们算进来会让一格永远等下去。
     */
    static boolean otherEntityInside(Run run, BlockPos target, EntityMaid maid) {
        for (Entity entity : run.level.getEntities(maid, new AABB(target))) {
            if (entity instanceof LivingEntity && entity.isAlive() && !entity.isSpectator()) {
                return true;
            }
        }
        return false;
    }

}
