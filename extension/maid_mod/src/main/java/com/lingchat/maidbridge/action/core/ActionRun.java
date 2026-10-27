package com.lingchat.maidbridge.action.core;


/**
 * 一次动作的状态机：游戏侧每 tick 填一份「观察」，这里只决定下一步做什么、什么时候收尾。
 *
 * <p>纯逻辑，能脱开 Minecraft 跑单测。所有改世界的事（写寻路记忆、坐下、睡觉）都在
 * {@link MaidActuator} 里，读游戏填观察也在那里，这里只做判断——这样「到位阈值、超时、
 * 卡住、睡着没睡着」这些容易写反的地方都能被单测钉住。
 *
 * <p>为什么判定在模组这一侧：LingChat 的插件脚本没有定时器、也存不下跨调用状态，
 * 「走过去用了多久、还差几格」只有常驻、每 tick 都在的模组算得出来。
 */
public final class ActionRun {
    /** 看向玩家保持多久（tick，20 = 1 秒）。 */
    public static final int LOOK_TICKS = 40;
    /** 多久没更靠近就算卡住。 */
    static final int STALL_TICKS = 80;
    /** 卡住时只要已经贴近到这个距离就当到了：床边那一格常常走不进去。 */
    static final int NEAR_ENOUGH = 4;
    /** 「走不到」这个标记要连着挂这么久才判失败：路径偶尔算不出来，别一惊一乍。 */
    static final int UNREACHABLE_TICKS = 40;

    private enum Phase {
        WALK,
        LOOK,
        /** 躺着（床上或地上），一直保持到被打断。 */
        LYING,
        /** 在主人前面探路，一直跟着走，直到被打断。 */
        SCOUT
    }

    /**
     * 躺在地上时被什么打断了。{@link #NONE} 表示还躺得住。
     *
     * <p>打断不是失败：她按环境自己爬起来了，收尾时如实说原因就行。
     */
    public record Interrupt(String code, String reason) {
        public static final Interrupt NONE = new Interrupt("", "");

        public boolean present() {
            return !code.isEmpty();
        }
    }

    /**
     * 游戏侧每 tick 填的观察，全是事实，不含判断。
     *
     * @param blocked 她现在动不了（坐着、被牵着、被骑着）
     * @param targetPresent 目标还在（玩家在线且同维度 / 床还在且空着）
     * @param cannotReach brain 里挂着「这个目标走不到」的标记
     * @param targetDistance 到目标的曼哈顿距离，与寻路判到位的口径一致
     * @param distance 到目标的直线距离，只用来报给模型看
     * @param interrupt 躺着与探路时打断她的那件事；其余动作用 {@link Interrupt#NONE}
     */
    public record Observation(
            boolean maidPresent,
            boolean blocked,
            boolean sleeping,
            boolean targetPresent,
            boolean cannotReach,
            int targetDistance,
            double distance,
            Interrupt interrupt) {
    }

    /** 下一步做什么。收尾时这个值没有意义。 */
    public enum Command {
        WALK,
        LOOK,
        LIE,
        SCOUT,
        HOLD
    }

    /** 状态机的结论：`finished` 为真表示这次动作结束，`status` 是 done / failed。 */
    public record Outcome(Command command, boolean finished, String status, String reasonCode, String reason) {
    }

    private final MaidAction action;
    private Phase phase;
    private int elapsed;
    private int bestDistance = Integer.MAX_VALUE;
    private int stallTicks;
    private int unreachableTicks;

    public ActionRun(MaidAction action) {
        this.action = action;
        this.phase = switch (action.kind()) {
            case LOOK_AT -> Phase.LOOK;
            case LIE_DOWN -> action.isGroundLie() ? Phase.LYING : Phase.WALK;
            case SCOUT -> Phase.SCOUT;
            default -> Phase.WALK;
        };
    }

    public int elapsedTicks() {
        return elapsed;
    }

    /** 已经躺下了：游戏侧据此决定要不要按住睡姿。 */
    public boolean lying() {
        return phase == Phase.LYING;
    }

    /** 推进一 tick。 */
    public Outcome tick(Observation observation) {
        elapsed++;
        // 她不在了就没什么可做的了：这条不管在哪个阶段都最优先
        if (!observation.maidPresent()) {
            return failed("maid_gone", "她不在这个世界里了");
        }
        return switch (phase) {
            case WALK -> walk(observation);
            case LOOK -> look(observation);
            case LYING -> lying(observation);
            case SCOUT -> scout(observation);
        };
    }

    private Outcome walk(Observation observation) {
        if (observation.blocked()) {
            return failed("maid_busy", "她现在动不了（坐着、被牵着或被骑着）");
        }
        if (!observation.targetPresent()) {
            return failed("target_gone", "目标不见了");
        }
        if (observation.targetDistance() <= action.arriveDistance()) {
            return arrived();
        }

        trackProgress(observation);
        if (stallTicks >= STALL_TICKS && observation.targetDistance() <= NEAR_ENOUGH) {
            // 床边那一格常常进不去，贴近了就不再靠近就当到了，免得白等
            return arrived();
        }
        if (unreachableTicks >= UNREACHABLE_TICKS) {
            return failed("unreachable", "她走不过去（路被挡住了）");
        }
        if (elapsed >= action.timeoutTicks()) {
            return failed("timeout", "她还没能走过去");
        }
        return new Outcome(Command.WALK, false, "", "", "");
    }

    /**
     * 到位了：躺下的动作换成躺姿并一直保持，其余动作当场收尾。
     *
     * <p>躺下不再盯「她睡不睡得着」：床上的睡姿由桥接按住的睡位维持，日程叫不醒她。
     */
    private Outcome arrived() {
        if (action.kind() == MaidAction.Kind.LIE_DOWN) {
            phase = Phase.LYING;
            return new Outcome(Command.LIE, false, "", "", "");
        }
        return done();
    }

    /** 看向玩家：只是转个头，保持一小会儿就结束，不需要完成事件。 */
    private Outcome look(Observation observation) {
        if (elapsed >= LOOK_TICKS || !observation.targetPresent()) {
            return done();
        }
        return new Outcome(Command.LOOK, false, "", "", "");
    }

    /**
     * 躺着：没有时长，没有到位线，一直保持到她被叫起来（新动作会顶掉这次）
     * 或环境让她躺不住（挨打、被抱走、泡进液体、身下没地、床没了）。
     */
    private Outcome lying(Observation observation) {
        if (observation.interrupt().present()) {
            return new Outcome(
                    Command.HOLD,
                    true,
                    "done",
                    observation.interrupt().code(),
                    observation.interrupt().reason());
        }
        return new Outcome(Command.LIE, false, "", "", "");
    }

    /**
     * 探路：没有时长也没有到位线，主人一边走她一边换点。
     *
     * <p>结束只有两种：主人让她做别的事（新动作顶掉这次），或者她撑不住（血不多了、
     * 被抱走、动不了、主人走了或走散了）。后几种收尾时如实说是哪件事。
     */
    private Outcome scout(Observation observation) {
        if (observation.interrupt().present()) {
            return new Outcome(
                    Command.HOLD,
                    true,
                    "done",
                    observation.interrupt().code(),
                    observation.interrupt().reason());
        }
        return new Outcome(Command.SCOUT, false, "", "", "");
    }

    private void trackProgress(Observation observation) {
        if (observation.targetDistance() < bestDistance) {
            bestDistance = observation.targetDistance();
            stallTicks = 0;
        } else {
            stallTicks++;
        }
        unreachableTicks = observation.cannotReach() ? unreachableTicks + 1 : 0;
    }

    private Outcome done() {
        return new Outcome(Command.HOLD, true, "done", "", "");
    }

    private Outcome failed(String reasonCode, String reason) {
        return new Outcome(Command.HOLD, true, "failed", reasonCode, reason);
    }
}
