package com.lingchat.maidbridge.action.construction;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.construction.MaidBuildController.Run;
import com.lingchat.maidbridge.action.core.MaidActuator;
import net.minecraft.server.MinecraftServer;

import static com.lingchat.maidbridge.action.construction.BuildLayer.*;
import static com.lingchat.maidbridge.action.construction.BuildMoves.*;
import static com.lingchat.maidbridge.action.construction.BuildPlanner.*;
import static com.lingchat.maidbridge.action.construction.MaidBuildController.*;

/**
 * 施工的运行相位：每 tick 只推进一件事。
 *
 * <pre>
 *            ┌──────────── 冷却结束 ────────────┐
 *            ↓                                  │
 *   SCAN ──有活──→ 下一步产生动作（MOVE / 冷却） │
 *     │                                           │
 *     └── 本层扫完 ──→ CLOSE ──决策表──→ SCAN ────┘
 *
 *   CLEAN 收尾（拆垫脚）优先于以上所有相位
 * </pre>
 *
 * <p>相位由运行状态推出来（{@link #of}），不另存一份意图：手上有没有动作、在不在冷却、
 * 是不是在收尾。进入与退出各有一个钩子，目前有三处需要做事——进 CLOSE 时冻结这一遍的
 * 统计，出 CLOSE 时丢掉快照，进 CLEAN 时重置收尾计时。
 *
 * <p>真正的状态机有两层在别处：一格的进度在 {@link CellFlow}，一层扫完的去向在
 * {@link BuildRules#endPass} 的决策表。
 */
enum BuildPhase {
    /** 扫本层、选格、推进一格。 */
    SCAN,
    /** 手上有一个动作在跑（走路、跑跳、垫脚、起跳）。 */
    MOVE,
    /** 放完一块之后的间隔冷却。 */
    COOLDOWN,
    /** 收尾：先离开垫脚块，再拆干净，最后推收尾事件。 */
    CLEAN,
    /** 本层扫完：数账、按决策表执行去向。 */
    CLOSE;

    /** 按运行状态判断这一 tick 该走哪个相位；收尾优先级最高。 */
    static BuildPhase of(Run run) {
        if (run.cleaning) {
            return CLEAN;
        }
        if (run.cooldown > 0) {
            return COOLDOWN;
        }
        if (run.move != null) {
            return MOVE;
        }
        return SCAN;
    }

    /** 切换相位：只有真的换了才跑退出与进入钩子。 */
    static void to(Run run, BuildPhase next) {
        if (run.phase == next) {
            return;
        }
        run.phase.exit(run);
        run.phase = next;
        next.enter(run);
    }

    /** 进入行为。 */
    void enter(Run run) {
        switch (this) {
            case CLEAN -> run.cleanTicks = 0;
            // 冻结这一遍的账：执行时只按同一份快照走，中途放下的方块不算进这一遍的进展
            case CLOSE -> run.passStats = measurePass(run);
            case SCAN, MOVE, COOLDOWN -> {
            }
        }
    }

    /** 退出行为。 */
    void exit(Run run) {
        switch (this) {
            case CLOSE -> run.passStats = null;
            case SCAN, MOVE, COOLDOWN, CLEAN -> {
            }
        }
    }

    /** 执行行为：推进这一相位一 tick。 */
    void tick(MinecraftServer server, Run run, EntityMaid maid) {
        switch (this) {
            case SCAN -> scan(server, run, maid);
            case MOVE -> {
                advance(run, maid);
                stop("动");
            }
            case COOLDOWN -> coolDown(run, maid);
            case CLEAN -> {
                stop("收尾");
                cleaning(server, run, maid);
            }
            case CLOSE -> endPass(server, run, run.passStats);
        }
    }

    /** 扫一格：先重排本层顺序，再从游标往后找能处理的；扫完就进收尾。 */
    private static void scan(MinecraftServer server, Run run, EntityMaid maid) {
        if (run.needOrder) {
            run.needOrder = false;
            orderLayer(run, maid);
        }
        if (next(run, maid)) {
            return;
        }
        // 这一 tick 的寻路额度用完了、游标还停在这一层中间：下个 tick 接着扫，
        // 不能当成「这一遍没进展」——那会让好好的格子被提前收进失败清单
        if (run.cursor < run.layerEnd) {
            stop("扫");
            return;
        }
        // 本层扫完，同一个 tick 里直接收尾，不空转一 tick
        to(run, CLOSE);
        CLOSE.tick(server, run, maid);
    }

    /** 冷却：只递减，手上还有走路目标的话继续朝它走。 */
    private static void coolDown(Run run, EntityMaid maid) {
        run.cooldown--;
        if (run.move != null && run.move.stand() != null) {
            MaidActuator.walkToPos(maid, run.move.stand(), 0);
        }
        stop("歇");
    }
}
