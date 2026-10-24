package com.lingchat.maidbridge.action.core;

import com.google.gson.JsonObject;
import com.lingchat.maidbridge.bridge.Envelope;
import java.util.Locale;

/**
 * 插件请求的一次动作：动作与目标的白名单校验，以及每个动作的元数据。
 *
 * <p>纯逻辑，不碰游戏，能脱开 Minecraft 跑单测。游戏语义（谁在睡觉、床在哪、走不走得
 * 过去）都在 {@link MaidActuator} 里，这里只判「这条请求说的是什么」。
 *
 * <p>参数全部走白名单，接口层面**没有任何接受坐标的入口**：回环端口没有鉴权，任何本地
 * 进程都能连上来，最坏也只能让女仆走两步、坐一下。
 */
public record MaidAction(Kind kind, Target target) {

    /** 插件能请求的动作。 */
    public enum Kind {
        GO_TO("go_to"),
        SIT("sit"),
        STAND("stand"),
        LIE_DOWN("lie_down"),
        LOOK_AT("look_at"),
        /** 到主人面朝的正前方去替他开路，一直跟着换点，直到被叫回来或自己撑不住。 */
        SCOUT("scout");

        private final String id;

        Kind(String id) {
            this.id = id;
        }

        public String id() {
            return id;
        }

        /** 认不出的动作给 null，由调用方转成拒绝原因。 */
        public static Kind byId(String id) {
            for (Kind kind : values()) {
                if (kind.id.equals(id)) {
                    return kind;
                }
            }
            return null;
        }
    }

    /** 移动类动作的目标。语义目标：没有坐标可给——感知只推地形与血量，给坐标等于请模型编数字。 */
    public enum Target {
        PLAYER("player"),
        BED("bed"),
        /** 就地躺下：不找床，也不走路。 */
        GROUND("ground");

        private final String id;

        Target(String id) {
            this.id = id;
        }

        public String id() {
            return id;
        }

        public static Target byId(String id) {
            for (Target target : values()) {
                if (target.id.equals(id)) {
                    return target;
                }
            }
            return null;
        }
    }

    /** 帧的解析结果：`action` 为 null 表示受理不了，`errorCode` / `error` 是要回给插件的原因。 */
    public record ParseResult(String maidId, MaidAction action, String errorCode, String error) {
        public boolean accepted() {
            return action != null;
        }

        static ParseResult rejected(String errorCode, String error) {
            return new ParseResult("", null, errorCode, error);
        }
    }

    /**
     * 解析 {@code maid_action} 的 data。
     *
     * <p>目标值认不出时退回默认（走 = 玩家、躺 = 最近的床）而不是报错：执行器的 schema
     * 校验不认 enum，模型塞进来的字符串本来就不可信；插件侧也做了同一套兜底。
     */
    public static ParseResult parse(JsonObject data) {
        String maidId = Envelope.string(data, "maid_id");
        if (maidId.isEmpty()) {
            return ParseResult.rejected("missing_maid_id", "没有指定女仆（maid_id 为空）");
        }
        String rawAction = Envelope.string(data, "action").toLowerCase(Locale.ROOT);
        Kind kind = Kind.byId(rawAction);
        if (kind == null) {
            return ParseResult.rejected("unsupported_action", "不支持的动作：" + rawAction);
        }
        return new ParseResult(maidId, new MaidAction(kind, parseTarget(kind, data)), "", "");
    }

    private static Target parseTarget(Kind kind, JsonObject data) {
        if (!needsTarget(kind)) {
            return null;
        }
        Target target = Target.byId(Envelope.string(data, "target").toLowerCase(Locale.ROOT));
        // 躺下只认床与地面：player 这种取值对它没有意义
        if (kind == Kind.LIE_DOWN) {
            return target == Target.BED || target == Target.GROUND ? target : defaultTarget(kind);
        }
        // 地面只对躺下有意义，走到「地面」说不通
        if (target == null || target == Target.GROUND) {
            return defaultTarget(kind);
        }
        return target;
    }

    private static boolean needsTarget(Kind kind) {
        return kind == Kind.GO_TO || kind == Kind.LIE_DOWN;
    }

    private static Target defaultTarget(Kind kind) {
        return kind == Kind.LIE_DOWN ? Target.BED : Target.PLAYER;
    }

    /** 曼哈顿距离多少算到位：与原版 {@code MoveToTargetSink} 按 closeEnoughDist 判到位的口径一致。 */
    public int arriveDistance() {
        if (kind == Kind.LIE_DOWN) {
            return target == Target.GROUND ? 0 : 2;
        }
        if (kind == Kind.GO_TO) {
            return target == Target.BED ? 2 : 3;
        }
        return 0;
    }

    /**
     * 走过去的超时（tick，20 = 1 秒）。按下最坏情况留够：绕路、开门、爬台阶。
     *
     * <p>不上场走路的动作（坐下、站起、就地躺下、看向玩家）给 0：它们的时长不归这里管
     * （看向玩家由 {@link ActionRun#LOOK_TICKS} 收尾）。探路也没有时长，它靠打断收尾。
     */
    public int timeoutTicks() {
        return switch (kind) {
            case GO_TO -> 600;
            case LIE_DOWN -> isGroundLie() ? 0 : 900;
            default -> 0;
        };
    }

    /**
     * 收尾时要不要推 {@code action_done}。
     *
     * <p>只有耗时动作需要：坐下/站起当场就完，看向玩家只是转个头，而插件会一直等一个
     * 不来的收尾——所以这个判断写反了，插件那边就会永远差一句旁白。
     */
    public boolean hasCompletionEvent() {
        return kind == Kind.GO_TO || kind == Kind.LIE_DOWN || kind == Kind.SCOUT;
    }

    /** 走过去的动作才会碰寻路与居家范围。就地躺下只动姿势。 */
    public boolean walks() {
        return kind == Kind.GO_TO || kind == Kind.SCOUT || (kind == Kind.LIE_DOWN && !isGroundLie());
    }

    /** 到主人前面探路：一个持续的状态，没有自己的时长。 */
    public boolean isScout() {
        return kind == Kind.SCOUT;
    }

    /** 就地躺下：不找床、不走路，一直躺着直到被叫起来或自己爬起来。 */
    public boolean isGroundLie() {
        return kind == Kind.LIE_DOWN && target == Target.GROUND;
    }

    /** 躺到床上：走过去之后按住的睡姿与按脚的躺姿走同一套保持逻辑。 */
    public boolean isBedLie() {
        return kind == Kind.LIE_DOWN && target == Target.BED;
    }

    /** 不论床上还是地上，这个动作就是在躺着。 */
    public boolean isLie() {
        return kind == Kind.LIE_DOWN;
    }

    public String actionId() {
        return kind.id();
    }

    public String targetId() {
        return target == null ? "" : target.id();
    }
}
