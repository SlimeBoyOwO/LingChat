package com.lingchat.maidbridge.action.core;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.action.attack.MaidAttackController;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.bridge.MessageRouter;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.action.mine.MaidMineController;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;

/**
 * 动作的单槽位持有者与每 tick 推进。
 *
 * <p>同一时刻只允许一个动作在跑：新动作顶掉旧的，并给旧的那次一个 cancelled 的交代——
 * 不然插件会一直等一个不来的收尾。不做队列、不做仲裁：玩家喊一句「过来」就该立刻过来，
 * 而不是先把手头的活干完。
 *
 * <p>计时用内部 tick 自增而不是墙钟：单人游戏按 Esc 暂停时 {@code ServerTickEvent.Post}
 * 不触发，于是「暂停不算超时」是天然的。
 *
 * <p>只在服务端主线程上跑（请求经 MessageRouter 排队、推进挂在 tick 事件上），所以这里
 * 的状态不需要任何同步。
 */
public final class MaidActionController {
    private static Run current;

    private static final class Run {
        final String maidId;
        final MaidAction action;
        final MaidActuator.Target target;
        final ActionRun machine;
        double distance;

        Run(String maidId, MaidAction action, MaidActuator.Target target) {
            this.maidId = maidId;
            this.action = action;
            this.target = target;
            this.machine = new ActionRun(action);
        }
    }

    private MaidActionController() {
    }

    /** {@code maid_action} 帧的落地：预检 → 顶掉旧动作 → 执行或挂上状态机。 */
    public static JsonObject start(MinecraftServer server, EntityMaid maid, MaidAction action) {
        // 追打、挖掘与动作都写寻路目标，同时跑她会在两个目标之间来回走
        MaidAttackController.cancel();
        MaidMineController.cancel(server);
        MaidActuator.Target target = MaidActuator.resolveTarget(maid, action);
        if (target.reject() != null) {
            return ActionFrames.rejected(action, target.reject().code(), target.reject().reason());
        }
        MaidActuator.Reject reject = MaidActuator.precheck(maid, action, target);
        if (reject != null) {
            return ActionFrames.rejected(action, reject.code(), reject.reason());
        }

        // 就地躺下：不走过去，当场换姿势，然后一直挂着——直到新动作顶掉它或她自己爬起来
        if (action.isGroundLie()) {
            String replaced = cancel(server);
            boolean stoodUp = MaidActuator.standUp(maid);
            MaidActuator.lieOnGround(maid);
            current = new Run(maid.getStringUUID(), action, target);
            return ActionFrames.accepted(action, true, stoodUp, replaced);
        }

        // 当场就完的两件事：先做，失败了还能如实拒绝（这时别去动那个正在跑的动作）
        if (action.kind() == MaidAction.Kind.SIT) {
            if (!MaidActuator.sit(maid)) {
                return ActionFrames.rejected(action, "sit_failed", "没能让她坐下");
            }
            String replaced = cancel(server);
            return ActionFrames.accepted(action, false, false, replaced);
        }
        if (action.kind() == MaidAction.Kind.STAND) {
            boolean stoodUp = MaidActuator.standUp(maid);
            String replaced = cancel(server);
            return ActionFrames.accepted(action, false, stoodUp, replaced);
        }

        String replaced = cancel(server);
        boolean stoodUp = false;
        if (action.walks()) {
            // 走过去之前先让她站直：坐着的时候她根本不会迈步
            stoodUp = MaidActuator.standUp(maid);
            MaidActuator.beginWalk(maid);
        }
        current = new Run(maid.getStringUUID(), action, target);
        // 耗时动作的结果稍后由 action_done 给出，插件据此知道自己还没到位
        return ActionFrames.accepted(action, action.hasCompletionEvent(), stoodUp, replaced);
    }

    /** 这个动作是不是正落在这只女仆身上：跟随行为给长任务让路时用。 */
    public static boolean isRunning(EntityMaid maid) {
        Run run = current;
        return run != null && maid != null && run.maidId.equals(maid.getStringUUID());
    }

    /**
     * 这只女仆是不是正躺在地上。
     *
     * <p>给它自己的姿势守护用（见 {@code EntityMaidPoseMixin}）：车万女仆的游泳管理器
     * 每 tick 会把不在水里的女仆的姿势改回站立，躺姿得在这里挡住。
     */
    public static boolean isGroundLying(EntityMaid maid) {
        Run run = current;
        return run != null && maid != null
                && run.action.isGroundLie()
                && run.maidId.equals(maid.getStringUUID());
    }

    /** 每个服务端 tick 推进一次（主线程）。 */
    public static void tick(MinecraftServer server) {
        Run run = current;
        if (run == null) {
            return;
        }
        EntityMaid maid = MaidQueries.findMaidById(server, run.maidId);
        ActionRun.Observation observation = MaidActuator.observe(maid, run.action, run.target);
        ActionRun.Outcome outcome = run.machine.tick(observation);
        run.distance = observation.distance();

        if (!outcome.finished() && maid != null) {
            act(maid, run, outcome.command());
        }
        if (outcome.finished()) {
            finish(run, outcome, maid);
        }
    }

    /**
     * 昏迷打断动作：收干净，并给还在等收尾的插件一个「她昏过去了」的交代。
     *
     * <p>不按「被新动作顶掉」说——那不是玩家又下了指令，而是她倒下了。
     */
    public static void cancelForUnconscious(MinecraftServer server) {
        Run run = current;
        if (run == null) {
            return;
        }
        EntityMaid maid = MaidQueries.findMaidById(server, run.maidId);
        current = null;
        if (maid != null) {
            if (run.action.isGroundLie()) {
                MaidActuator.wakeUp(maid);
            }
            MaidActuator.stopWalking(maid);
            MaidActuator.stopLooking(maid);
        }
        if (run.action.hasCompletionEvent()) {
            broadcast(run, "failed", "unconscious", "她昏过去了");
        }
    }

    /** 停服或换存档时清干净。 */
    public static void reset() {
        current = null;
    }

    /**
     * 换工作模式前把没做完的动作收掉，返回被顶掉的动作名（没有则为空串）。
     *
     * <p>这个控制器每 tick 写一次寻路目标，与工作模式自己的寻路会互相顶：不收掉的话，
     * 她会在「走过去」和「去干活」两个目标之间来回跳。
     */
    public static String cancelCurrent(MinecraftServer server) {
        return cancel(server);
    }

    private static void act(EntityMaid maid, Run run, ActionRun.Command command) {
        switch (command) {
            case WALK -> {
                if (run.target.isBed()) {
                    MaidActuator.walkToBed(maid, run.target.bedHead(), run.action.arriveDistance());
                } else {
                    ServerPlayer player = MaidQueries.owner(maid);
                    if (player != null) {
                        MaidActuator.walkToPlayer(maid, player, run.action.arriveDistance());
                    }
                }
            }
            case LOOK -> {
                ServerPlayer player = MaidQueries.owner(maid);
                if (player != null) {
                    MaidActuator.lookAtPlayer(maid, player);
                }
            }
            case SLEEP -> MaidActuator.sleepIn(maid, run.target.bedHead());
            case LIE -> MaidActuator.holdLying(maid);
            case HOLD -> {
            }
        }
    }

    private static void finish(Run run, ActionRun.Outcome outcome, EntityMaid maid) {
        current = null;
        if (maid != null) {
            // 地上躺着收尾时要真的把她扶起来；床上的睡姿不归这里管
            if (run.action.isGroundLie()) {
                MaidActuator.wakeUp(maid);
            }
            MaidActuator.stopWalking(maid);
            MaidActuator.stopLooking(maid);
        }
        // 没有完成事件的动作（坐下、站起、看一会儿）收尾是静默的
        if (run.action.hasCompletionEvent()) {
            broadcast(run, outcome.status(), outcome.reasonCode(), outcome.reason());
        }
    }

    /** 顶掉在跑的动作，返回被顶掉的那个动作名（没有则为空串）。 */
    private static String cancel(MinecraftServer server) {
        Run run = current;
        if (run == null) {
            return "";
        }
        EntityMaid maid = MaidQueries.findMaidById(server, run.maidId);
        finish(run, new ActionRun.Outcome(ActionRun.Command.HOLD, true, "cancelled", "replaced", "被新动作顶掉了"), maid);
        return run.action.actionId();
    }

    private static void broadcast(Run run, String status, String reasonCode, String reason) {
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge == null || !bridge.hasClients()) {
            return;
        }
        bridge.broadcastEvent(ActionFrames.done(
                run.maidId, run.action, status, reasonCode, reason, run.machine.elapsedTicks(), run.distance));
    }
}
