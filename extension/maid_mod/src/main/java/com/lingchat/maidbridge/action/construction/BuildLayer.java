package com.lingchat.maidbridge.action.construction;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.construction.BuildRules.CellState;
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

/**
 * 一层的进度与一层走完后的去向：决策表的执行侧。
 *
 * <p>一格自己的状态迁移在 {@link CellFlow}，这里只管按层统计、执行动作管线。
 */
final class BuildLayer {
    /** 这一格归这一遍管：还开着；门留到别的格子都放好之后再装。 */
    static boolean inPass(Cell cell, boolean doorsOnly) {
        return cell.state.open() && (doorsOnly || cell.block != Blocks.OAK_DOOR);
    }

    /** 这一格被活物占着，还没到重试的时候。 */
    static boolean held(Cell cell, ServerLevel level) {
        return cell.holdUntil > level.getGameTime();
    }

    /**
     * 数这一遍的账：有没有进展、连续几遍没进展、本层还剩多少开格。
     *
     * <p>只读世界并更新计数，不决定去哪；去向交给纯规则的 {@link BuildRules#endPass}。
     * 进入收尾相位时数一次，执行时按同一份快照走，中间不重算。
     */
    static PassStats measurePass(Run run) {
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
        return new PassStats(
                run.placed >= run.total,
                layerOpen,
                layerOpen && waitHeld,
                progress,
                run.stallPasses,
                run.retried,
                nextLayerY(run, doorsOnly) != null,
                hasDeferred(run));
    }

    /** 按决策表给出的去向执行这一遍的收尾。 */
    static void endPass(MinecraftServer server, Run run, PassStats stats) {
        applyEnd(server, run, BuildRules.endPass(stats));
    }

    /** 按纯规则给出的去向执行：动作按顺序来，收工之后的不再执行。 */
    static void applyEnd(MinecraftServer server, Run run, PassEnd end) {
        stop("层:" + end.text());
        for (Action action : end.actions()) {
            if (perform(server, run, action)) {
                return;
            }
        }
        run.cooldown = PASS_BREAK_TICKS;
    }

    /**
     * 执行一个收尾动作；返回真表示收工。
     *
     * <p>写成 switch 表达式：新增动作时编译器会逼着这里补一行。以前是两段带 default 的
     * switch，漏接的动作静默通过，施工会在原地反复空转。
     */
    private static boolean perform(MinecraftServer server, Run run, Action action) {
        return switch (action) {
            case RESCAN -> {
                restartPass(run);
                yield false;
            }
            case NEXT_LAYER -> {
                switchLayer(run);
                yield false;
            }
            case RETRY_ROUND -> {
                retryRound(run);
                yield false;
            }
            case DEFER_LAYER -> {
                deferLayer(run);
                yield false;
            }
            case SETTLE_LAYER -> {
                settleLayer(run);
                yield false;
            }
            case FINISH -> {
                finish(server, run, "");
                yield true;
            }
        };
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
            CellFlow.park(run, cell, cell.failure);
        }
    }

    /**
     * 整张图纸都过完了：把之前没放上的格子放回来再试一轮。
     *
     * <p>别的方块放好之后，原来缺依托、被别的方块挡住的格子常常就能放了。每一格只补试
     * 一次：补过还是不行的由 {@link CellFlow#fail} 记账，不会在同一个地方反复打转。
     */
    static boolean retryRound(Run run) {
        if (!hasDeferred(run)) {
            return false;
        }
        for (Cell cell : run.cells) {
            if (cell.state == CellState.DEFERRED) {
                CellFlow.retry(run, cell);
            }
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
            CellFlow.fail(run, cell, cell.failure);
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

    /** 这一遍重头再扫：顺序按她此刻站的位置重排，由远及近。 */
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
     * <p>只看「放好了」，不看「收账了」：有格子因为够不着、被挡或液体放弃时，门先不装，
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
