package com.lingchat.maidbridge.perception.world;

import com.google.gson.JsonObject;
import com.lingchat.maidbridge.perception.EventFrames;
import java.util.ArrayList;
import java.util.List;

/**
 * 感知的变化判定：上一帧 + 这一帧 → 该推给插件的事件。
 *
 * <p>纯逻辑，不碰游戏，能脱开 Minecraft 跑单测。只推**结构化字段**，中文措辞留给
 * 插件（Python）——游戏语义留在 Java，自然语言理解留在 Python。
 *
 * <p>为什么判定在模组这一侧：LingChat 的插件脚本没有定时器、也存不下跨调用状态，
 * 「跟上一次比」这件事只有常驻、有 tick、记得住上一帧的模组做得了。
 */
public final class PerceptionDiff {
    /** 换群系的最短间隔（tick）：鞘翅赶路横穿二十个群系，不该刷二十条旁白；一分钟一条。 */
    public static final long BIOME_COOLDOWN_TICKS = 1200L;

    /** 血量跌破阈值时报一次，回到「阈值 × 这个系数」以上才允许再报。 */
    private static final double LOW_HEALTH_REARM = 1.2;

    private static final long DAY_TICKS = 24000L;
    /** 原版入夜时刻：世界时间的这个点之前算白天。 */
    private static final long NIGHT_START = 13000L;

    private PerceptionDiff() {
    }

    /** 跨帧的记忆：只看相邻两帧判不出来的东西（冷静期、血量回滞）。 */
    public static final class State {
        /** Long.MIN_VALUE 表示「还没报过」；比较时用加法，免得减法溢出。 */
        private long lastBiomeTick = Long.MIN_VALUE;
        private boolean playerLow;
        private boolean maidLow;

        /** 换了监控对象或重新接上时清掉，免得拿旧世界的状态去比。 */
        public void reset() {
            lastBiomeTick = Long.MIN_VALUE;
            playerLow = false;
            maidLow = false;
        }
    }

    /**
     * 这一帧该推的事件（可能一条都没有）。
     *
     * @param nowTick 当前世界 tick，用来做冷静期
     * @param lowHealthRatio 血量低于这个比例算「快撑不住了」
     */
    public static List<JsonObject> events(
            PerceptionSnapshot prev,
            PerceptionSnapshot next,
            State state,
            long nowTick,
            double lowHealthRatio) {
        List<JsonObject> frames = new ArrayList<>();

        // 没有上一帧，或换了监控对象：先交代「现在是什么样」，这一帧不叠加别的
        if (prev == null || !prev.entityId().equals(next.entityId())) {
            state.reset();
            state.lastBiomeTick = nowTick;
            frames.add(snapshotFrame(next));
            return frames;
        }

        // 昏迷时她什么也感知不到：只保留重连时那一份现状，世界变化全部不推。
        // 挨打、死亡、上床与醒来另有专门事件
        if ("unconscious".equals(next.posture())) {
            return frames;
        }

        boolean dimensionChanged = !prev.dimension().equals(next.dimension());
        if (dimensionChanged) {
            // 换维度时群系必然也变，只报维度那一条
            state.lastBiomeTick = nowTick;
            JsonObject frame = EventFrames.base("dimension_change", next.entityId());
            frame.addProperty("from_dimension", prev.dimension());
            frame.addProperty("to_dimension", next.dimension());
            frames.add(frame);
        } else if (!prev.biome().equals(next.biome())
                && nowTick >= state.lastBiomeTick + BIOME_COOLDOWN_TICKS) {
            state.lastBiomeTick = nowTick;
            JsonObject frame = EventFrames.base("biome_change", next.entityId());
            frame.addProperty("from_biome", prev.biome());
            frame.addProperty("biome", next.biome());
            frames.add(frame);
        }

        // 没有天空的维度（下界 / 末地）不报天气与昼夜：那里这两样没有意义
        if (prev.hasSky() && next.hasSky()) {
            if (prev.raining() != next.raining() || prev.thundering() != next.thundering()) {
                JsonObject frame = EventFrames.base("weather_change", next.entityId());
                frame.addProperty("raining", next.raining());
                frame.addProperty("thundering", next.thundering());
                frames.add(frame);
            }
            if (!phase(prev).equals(phase(next))) {
                JsonObject frame = EventFrames.base("time_phase_change", next.entityId());
                frame.addProperty("phase", phase(next));
                frames.add(frame);
            }
        }

        addLowHealth(frames, next, state, lowHealthRatio);
        return frames;
    }

    /** 血量告警：跌破阈值那一下报一次（边沿触发），缓过来才允许再报。 */
    private static void addLowHealth(
            List<JsonObject> frames,
            PerceptionSnapshot next,
            State state,
            double lowHealthRatio) {
        JsonObject player = lowHealthFrame(
                "player", next, next.playerHealth(), next.playerMaxHealth(), state.playerLow, lowHealthRatio);
        if (player != null) {
            state.playerLow = true;
            frames.add(player);
        } else if (state.playerLow && recovered(next.playerHealth(), next.playerMaxHealth(), lowHealthRatio)) {
            state.playerLow = false;
        }

        JsonObject maid = lowHealthFrame(
                "maid", next, next.maidHealth(), next.maidMaxHealth(), state.maidLow, lowHealthRatio);
        if (maid != null) {
            state.maidLow = true;
            frames.add(maid);
        } else if (state.maidLow && recovered(next.maidHealth(), next.maidMaxHealth(), lowHealthRatio)) {
            state.maidLow = false;
        }
    }

    private static JsonObject lowHealthFrame(
            String who,
            PerceptionSnapshot next,
            double health,
            double maxHealth,
            boolean alreadyReported,
            double lowHealthRatio) {
        if (alreadyReported || maxHealth <= 0 || health / maxHealth >= lowHealthRatio) {
            return null;
        }
        JsonObject frame = EventFrames.base("low_health", next.entityId());
        frame.addProperty("who", who);
        frame.addProperty("name", who.equals("maid") ? next.maidName() : next.playerName());
        frame.addProperty("health", EventFrames.round(health));
        frame.addProperty("max_health", EventFrames.round(maxHealth));
        return frame;
    }

    private static boolean recovered(double health, double maxHealth, double lowHealthRatio) {
        return maxHealth <= 0 || health / maxHealth > lowHealthRatio * LOW_HEALTH_REARM;
    }

    private static JsonObject snapshotFrame(PerceptionSnapshot s) {
        JsonObject frame = EventFrames.base("state_snapshot", s.entityId());
        frame.addProperty("dimension", s.dimension());
        frame.addProperty("biome", s.biome());
        // 没有天空的维度就别提昼夜与天气：那里这两样不存在
        frame.addProperty("sky", s.hasSky());
        if (s.hasSky()) {
            frame.addProperty("phase", phase(s));
        }
        frame.addProperty("raining", s.raining());
        frame.addProperty("thundering", s.thundering());
        frame.addProperty("player_name", s.playerName());
        frame.addProperty("player_health", EventFrames.round(s.playerHealth()));
        frame.addProperty("player_max_health", EventFrames.round(s.playerMaxHealth()));
        frame.addProperty("maid_name", s.maidName());
        frame.addProperty("maid_health", EventFrames.round(s.maidHealth()));
        frame.addProperty("maid_max_health", EventFrames.round(s.maidMaxHealth()));
        // 姿势是持久状态：接上那一刻她可能已经坐着或躺在地上，接下来的变化另由 posture_change 交代
        frame.addProperty("posture", s.posture());
        return frame;
    }

    private static String phase(PerceptionSnapshot s) {
        return s.dayTime() % DAY_TICKS < NIGHT_START ? "day" : "night";
    }
}
