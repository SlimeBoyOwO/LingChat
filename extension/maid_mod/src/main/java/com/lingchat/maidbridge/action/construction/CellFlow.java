package com.lingchat.maidbridge.action.construction;

import com.lingchat.maidbridge.action.construction.BuildRules.CellState;
import com.lingchat.maidbridge.action.construction.BuildRules.Failure;
import com.lingchat.maidbridge.action.construction.MaidBuildController.Cell;
import com.lingchat.maidbridge.action.construction.MaidBuildController.Run;

/**
 * 一格的进度状态机：这是唯一能改 {@link Cell#state} 的地方。
 *
 * <pre>
 * PENDING ──┐
 *           ├─→ DONE                放好了，或本来就已经符合
 * RETRYING ─┘
 * PENDING/RETRYING ──→ FAILED      液体、缺料，或换成别的方块占着
 * PENDING ──→ DEFERRED ──→ RETRYING ──→ FAILED   搁置、补试、仍不行
 * </pre>
 *
 * <p>每次迁移分两步：先跑旧状态的退出行为，再跑新状态的进入行为。退出行为负责把为它
 * 占的落脚点和手上这一格放开；进入行为负责记账与重置。判定「放不放得下」不在这里，
 * 这里只接收别人观察到的结局。
 *
 * <p>调用点不再区分「临时记一下」「搁置」「收账」各要写哪些字段：那三套规则以前散在
 * {@code temporary}/{@code defer}/{@code settle} 三个函数、三十多个调用点里，同一个失败
 * 原因在不同位置得到不同处置。现在一律经本类，规则只有下面这一份。
 */
final class CellFlow {
    /** 同一格换过这么多个站位都不行，就先搁到补试轮。 */
    private static final int MAX_STAND_TRIES = 4;

    private CellFlow() {
    }

    /** 这一格放好了。 */
    static void done(Run run, Cell cell) {
        transition(run, cell, CellState.DONE, null);
    }

    /** 这一遍先放着：记下原因，下个 tick 或下一遍还会再试。 */
    static void hold(Run run, Cell cell, Failure cause) {
        if (!cell.state.open()) {
            return;
        }
        cell.failure = cause;
        cell.attempted = true;
    }

    /**
     * 这一遍没辙：站位换过几轮还不行就先搁到补试轮，否则只记原因等下个 tick。
     *
     * <p>以前这是 BuildPlanner 里的 giveUp：临时记一笔，再用「试过几个站位」决定要不要搁置。
     * 这条升级规则现在归状态机管。
     */
    static void holdThenPark(Run run, Cell cell, Failure cause) {
        hold(run, cell, cause);
        if (cell.nav.triedStands.size() >= MAX_STAND_TRIES) {
            park(run, cell, cause);
        }
    }

    /**
     * 搁到整张图纸过完之后的补试轮。
     *
     * <p>补试那一轮再放弃就直接收账，不再搁一次。
     */
    static void park(Run run, Cell cell, Failure cause) {
        if (!cell.state.open()) {
            return;
        }
        CellState next = cell.state == CellState.RETRYING ? CellState.FAILED : CellState.DEFERRED;
        transition(run, cell, next, cause);
    }

    /** 有定论：收进失败清单。缺料单独记，收尾时交给插件。 */
    static void fail(Run run, Cell cell, Failure cause) {
        if (!cell.state.open()) {
            return;
        }
        transition(run, cell, CellState.FAILED, cause);
    }

    /** 补试轮把搁置的格子放回来：周围变样了，上一遍的尝试全部作废重来。 */
    static void retry(Run run, Cell cell) {
        if (cell.state == CellState.DEFERRED) {
            transition(run, cell, CellState.RETRYING, null);
        }
    }

    /**
     * 允许的迁移：搁置还没定论，可以转补试或收账；完成与失败的格子只能再往完成走。
     *
     * <p>不能拿 open() 当判据：搁置不是开格，但它正是补试轮要重新打开的状态。
     */
    static boolean allowed(CellState from, CellState to) {
        if (from == to) {
            return false;
        }
        if (from == CellState.DONE || from == CellState.FAILED) {
            return to == CellState.DONE;
        }
        return true;
    }

    /**
     * 唯一的迁移入口：退出旧状态，换状态，进入新状态。
     *
     * <p>已有定论的格子只能往 DONE 走，其余迁移一律忽略，避免收账之后又被别的路径改回来。
     */
    private static void transition(Run run, Cell cell, CellState next, Failure cause) {
        CellState from = cell.state;
        if (!allowed(from, next)) {
            return;
        }
        if (cause != null) {
            cell.failure = cause;
        }
        exit(run, cell);
        cell.state = next;
        enter(run, cell, next);
    }

    /** 退出行为：放开为这一格占的落脚点，并把手上的这一格交出去。 */
    private static void exit(Run run, Cell cell) {
        if (run.standFor == cell) {
            run.standFor = null;
        }
        if (run.active == cell) {
            run.active = null;
        }
    }

    /**
     * 进入行为：按新状态记账或重置。
     *
     * <p>返回值不用，写成 switch 表达式只是为了强制穷尽：多一个状态时编译器会报错。
     */
    private static CellState enter(Run run, Cell cell, CellState next) {
        return switch (next) {
            case DONE -> {
                run.built.add(cell.pos);
                run.placed++;
                yield next;
            }
            case FAILED -> {
                if (cell.failure == Failure.SHORT_OF_MATERIAL) {
                    run.missing.merge(cell.itemId, 1, Integer::sum);
                } else {
                    run.failed.merge(failureCode(cell), 1, Integer::sum);
                }
                yield next;
            }
            case RETRYING -> {
                // 周围已经变样了，之前试过的站位不一定还成立；垫过的依托与铺过的路也重新给一次
                cell.waits = 0;
                cell.propped = false;
                cell.navFails = 0;
                cell.nav.triedStands.clear();
                cell.nav.badBlocks.clear();
                cell.nav.steps = 0;
                cell.nav.bridges = 0;
                cell.nav.drops = 0;
                yield next;
            }
            case PENDING, DEFERRED -> next;
        };
    }

    /**
     * 这一格收账时记哪个原因。
     *
     * <p>从没真正试过的格子（依托面还没出现、一遍没轮到）不该拿默认的「没站位」顶数：
     * 收尾时那样会把「还没盖完」说成「旁边没地方站」。
     */
    private static String failureCode(Cell cell) {
        return cell.attempted ? cell.failure.code : "incomplete";
    }
}
