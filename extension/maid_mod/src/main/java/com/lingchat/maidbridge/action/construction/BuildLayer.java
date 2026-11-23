package com.lingchat.maidbridge.action.construction;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.construction.BuildRules.CellState;
import com.lingchat.maidbridge.action.construction.BuildRules.Failure;
import com.lingchat.maidbridge.action.construction.BuildRules.PassEnd;
import com.lingchat.maidbridge.action.construction.BuildRules.PassStats;
import java.util.Comparator;
import net.minecraft.core.BlockPos;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;

import com.lingchat.maidbridge.action.construction.BuildRules.*;
import com.lingchat.maidbridge.action.construction.MaidBuildController.Cell;
import com.lingchat.maidbridge.action.construction.MaidBuildController.Run;
import static com.lingchat.maidbridge.action.construction.MaidBuildController.*;

/** 一格的进度与一层走完后的去向：纯规则的执行侧。 */
final class BuildLayer {
    /** 这一格归这一遍管：还开着；门留到别的格子都放好之后再装。 */
    static boolean inPass(Cell cell, boolean doorsOnly) {
        return cell.state.open() && (doorsOnly || cell.block != Blocks.OAK_DOOR);
    }

    /** 这一格被活物占着，还没到重试的时候。 */
    static boolean held(Cell cell, ServerLevel level) {
        return cell.holdUntil > level.getGameTime();
    }

    static void markDone(Run run, Cell cell) {
        if (cell.state == CellState.DONE) {
            return;
        }
        cell.state = CellState.DONE;
        run.built.add(cell.pos);
        run.placed++;
        releaseStand(run, cell);
    }

    /** 这一格不用再为它守着了：它占着的落脚点可以正常放了。 */
    static void releaseStand(Run run, Cell cell) {
        if (run.standFor == cell) {
            run.standFor = null;
        }
    }

    /** 缺料：这一轮不再为这一格努力，但把还差多少件记下来交给插件。 */
    static void missing(Run run, Cell cell) {
        if (!cell.state.open()) {
            return;
        }
        cell.state = CellState.FAILED;
        cell.failure = Failure.SHORT_OF_MATERIAL;
        releaseStand(run, cell);
        run.missing.merge(cell.itemId, 1, Integer::sum);
    }

    /** 这一遍还不放弃：记下原因，等这一层整轮没有新进展时再一起收。 */
    static void temporary(Run run, Cell cell, Failure failure) {
        cell.failure = failure;
        cell.attempted = true;
    }

    /**
     * 这一格收账时记哪个原因。
     *
     * <p>从没真正试过的格子（依托面还没出现、一遍没轮到）不该拿默认的「没站位」顶数：
     * 收尾时那样会把「还没盖完」说成「旁边没地方站」。
     */
    static String failureCode(Cell cell) {
        return cell.attempted ? cell.failure.code : "incomplete";
    }

    /** 这一格有定论了：记进失败清单，收尾时交给插件。 */
    static void settle(Run run, Cell cell, Failure failure) {
        if (!cell.state.open()) {
            return;
        }
        releaseStand(run, cell);
        cell.state = CellState.FAILED;
        cell.failure = failure;
        run.failed.merge(failureCode(cell), 1, Integer::sum);
    }

    /**
     * 这一轮先放一放：别的格子放好之后，缺依托、被挡住的常常就能放了。
     *
     * <p>整张图纸过完一遍再回头补试，补试过还是不行的才由 {@link #settle} 记账。
     */
    static void defer(Run run, Cell cell, Failure failure) {
        if (!cell.state.open()) {
            return;
        }
        releaseStand(run, cell);
        cell.failure = failure;
        // 补试那一轮再放弃就直接算失败，不再搁一次
        cell.state = cell.state == CellState.RETRYING ? CellState.FAILED : CellState.DEFERRED;
        if (cell.state == CellState.FAILED) {
            run.failed.merge(failureCode(cell), 1, Integer::sum);
        }
    }

    /**
     * 一层走完：这一层还有没盖的、这一遍又有新进展，就再走一遍；连续几遍没有进展
     * 才把这层剩下的格子收进失败清单，换下一层。
     */
    static void endLayer(MinecraftServer server, Run run) {
        boolean doorsOnly = onlyDoorsLeft(run);
        // 垫脚也算这一遍的进展：爬上去要好几步，不能爬一半就被当成走不到收掉
        boolean progress = run.placed + run.stepPlaced > run.layerPassPlaced;
        boolean held = openCellsHeld(run, doorsOnly);
        // 全被活物占着就等一会儿；但只等有限几次，不然一只不动的活物能把整座房子耗死
        long now = run.level.getGameTime();
        boolean waitHeld = held;
        if (held) {
            if (run.heldSince == 0) {
                run.heldSince = now;
            } else if (now - run.heldSince >= HOLD_GIVEUP_TICKS) {
                if (run.stallPasses == 0) {
                    LOGGER.info("施工：本层被活物占着超过 {} 秒，先搁置", HOLD_GIVEUP_TICKS / 20);
                }
                waitHeld = false;
            }
        } else {
            run.heldSince = 0;
        }
        if (progress) {
            run.layerPassPlaced = run.placed + run.stepPlaced;
            run.stallPasses = 0;
        } else if (waitHeld) {
            // 剩下的都在等活物挪开：先歇一会儿，不算这一层失败
            run.stallPasses = 0;
        } else {
            run.stallPasses++;
        }
        boolean layerOpen = layerHasOpen(run, doorsOnly);
        PassEnd end = BuildRules.endPass(new PassStats(
                run.placed >= run.total,
                layerOpen,
                layerOpen && waitHeld,
                progress,
                run.stallPasses,
                run.retried,
                nextLayerY(run, doorsOnly) != null,
                hasDeferred(run)));
        applyEnd(server, run, end);
    }

    /** 按纯规则给出的去向执行：先搁置或收账，再换层、补试或收工。 */
    static void applyEnd(MinecraftServer server, Run run, PassEnd end) {
        stop("层:" + end.first() + "/" + end.second());
        switch (end.first()) {
            case DEFER_LAYER -> deferLayer(run);
            case SETTLE_LAYER -> settleLayer(run);
            case RESCAN -> restartPass(run);
            // 本层已经没有开格时，换层是决策表的第一个动作，不能只在第二个动作里处理
            case NEXT_LAYER -> switchLayer(run);
            case FINISH -> {
                finish(server, run, "");
                return;
            }
            default -> {
            }
        }
        switch (end.second()) {
            case NEXT_LAYER -> switchLayer(run);
            case RETRY_ROUND -> retryRound(run);
            case FINISH -> {
                finish(server, run, "");
                return;
            }
            default -> {
            }
        }
        run.cooldown = PASS_BREAK_TICKS;
    }

    /** 换到下一处还有开格的层；找不到就留在原地。 */
    static void switchLayer(Run run) {
        Integer y = nextLayerY(run, onlyDoorsLeft(run));
        if (y != null) {
            setLayer(run, y);
        }
    }

    static boolean layerHasOpen(Run run, boolean doorsOnly) {
        for (int i = run.layerStart; i < run.layerEnd; i++) {
            Cell cell = run.cells.get(i);
            if (!inPass(cell, doorsOnly)) {
                continue;
            }
            return true;
        }
        return false;
    }

    /** 这一层剩下的格子是不是全在等活物挪开。 */
    static boolean openCellsHeld(Run run, boolean doorsOnly) {
        boolean any = false;
        for (int i = run.layerStart; i < run.layerEnd; i++) {
            Cell cell = run.cells.get(i);
            if (!inPass(cell, doorsOnly)) {
                continue;
            }
            any = true;
            if (!held(cell, run.level)) {
                return false;
            }
        }
        return any;
    }

    /** 这一层暂时收不起来：先记着，别的层放完之后还会回来补。 */
    static void deferLayer(Run run) {
        boolean doorsOnly = onlyDoorsLeft(run);
        for (int i = run.layerStart; i < run.layerEnd; i++) {
            Cell cell = run.cells.get(i);
            if (!inPass(cell, doorsOnly)) {
                continue;
            }
            defer(run, cell, cell.failure);
        }
    }

    /**
     * 整张图纸都过完了：把之前没放上的格子放回来再试一轮。
     *
     * <p>别的方块放好之后，原来缺依托、被别的方块挡住的格子常常就能放了。每一格只补试
     * 一次：补过还是不行的由 {@link #settle} 记账，不会在同一个地方反复打转。
     */
    static boolean retryRound(Run run) {
        if (!hasDeferred(run)) {
            return false;
        }
        for (Cell cell : run.cells) {
            if (cell.state != CellState.DEFERRED) {
                continue;
            }
            cell.state = CellState.RETRYING;
            cell.waits = 0;
            // 周围已经变样了，之前试过的站位不一定还成立；垫过的依托与铺过的路也重新给一次
            cell.triedStands.clear();
            cell.propped = false;
            cell.bridges = 0;
            cell.drops = 0;
        }
        run.retried = true;
        return resetLayer(run);
    }

    static boolean hasDeferred(Run run) {
        for (Cell cell : run.cells) {
            if (cell.state == CellState.DEFERRED) {
                return true;
            }
        }
        return false;
    }

    static void settleLayer(Run run) {
        boolean doorsOnly = onlyDoorsLeft(run);
        for (int i = run.layerStart; i < run.layerEnd; i++) {
            Cell cell = run.cells.get(i);
            if (!inPass(cell, doorsOnly)) {
                continue;
            }
            settle(run, cell, cell.failure);
        }
    }

    /**
     * 下一处还有开格的层；没有返回 null。
     *
     * <p>先看后面的层；只剩门时再回到门所在的层，哪怕那一层已经走过。
     */
    static Integer nextLayerY(Run run, boolean doorsOnly) {
        for (int i = run.layerEnd; i < run.cells.size(); i++) {
            Cell cell = run.cells.get(i);
            if (!inPass(cell, doorsOnly)) {
                continue;
            }
            return cell.pos.getY();
        }
        if (doorsOnly) {
            for (Cell cell : run.cells) {
                if (cell.state.open() && cell.block == Blocks.OAK_DOOR) {
                    return cell.pos.getY();
                }
            }
        }
        return null;
    }

    static boolean resetLayer(Run run) {
        boolean doorsOnly = onlyDoorsLeft(run);
        for (Cell cell : run.cells) {
            if (!inPass(cell, doorsOnly)) {
                continue;
            }
            setLayer(run, cell.pos.getY());
            return true;
        }
        return false;
    }

    static void setLayer(Run run, int y) {
        int start = 0;
        while (start < run.cells.size() && run.cells.get(start).pos.getY() < y) {
            start++;
        }
        int end = start;
        while (end < run.cells.size() && run.cells.get(end).pos.getY() == y) {
            end++;
        }
        run.layerStart = start;
        run.layerEnd = end;
        run.active = null;
        run.standFor = null;
        run.heldSince = 0;
        for (int i = start; i < end; i++) {
            run.cells.get(i).waits = 0;
        }
        restartPass(run);
        run.layerPassPlaced = run.placed + run.stepPlaced;
        run.stallPasses = 0;
    }

    /** 这一遍重头再扫：顺序按她此刻站的位置重排，先做近的。 */
    static void restartPass(Run run) {
        run.cursor = run.layerStart;
        run.needOrder = true;
    }

    /**
     * 把本层还没做的格子按她现在的位置重排，由远及近。
     *
     * <p>先放离她远的那一头，边放边往自己这边退：反过来先放近的，会在身后留下一圈
     * 没放的格子，人退不出去，也没法从外侧绕。图纸按锚点附近排过一遍，她走到哪它不会
     * 跟着变；每遍重扫时重排一次，这一层两头各剩几格时也不用横穿整栋楼。
     */
    static void orderLayer(Run run, EntityMaid maid) {
        BlockPos here = maid.blockPosition();
        run.cells.subList(run.layerStart, run.layerEnd).sort(
                Comparator.comparingInt((Cell cell) -> cell.state.open() ? 0 : 1)
                        .thenComparingInt(cell -> supported(run.level, cell.pos) ? 0 : 1)
                        .thenComparingDouble(cell -> -cell.pos.distSqr(here)));
    }

    /**
     * 除了门以外的格子都已经放好：这时候才装门，施工期间门洞保持敞开。
     *
     * <p>只看「放好了」，不看「settled」：有格子因为够不着、被挡或液体放弃时，门先不装，
     * 她之后回来补料、接着盖还能从门洞进出；否则半成品把门口封死，人就困在里外了。
     */
    static boolean onlyDoorsLeft(Run run) {
        for (Cell cell : run.cells) {
            if (cell.state == CellState.DONE) {
                continue;
            }
            if (cell.block != Blocks.OAK_DOOR) {
                return false;
            }
        }
        return true;
    }

    static boolean supported(ServerLevel level, BlockPos pos) {
        return solidCell(level, pos.below());
    }

    /** 这一格是不是实心方块：垫脚块要立在它上面，或者它自己就得先垫出来。 */
    static boolean solidCell(ServerLevel level, BlockPos pos) {
        BlockState state = level.getBlockState(pos);
        return !state.isAir()
                && state.getFluidState().isEmpty()
                && !state.getCollisionShape(level, pos).isEmpty();
    }

}
