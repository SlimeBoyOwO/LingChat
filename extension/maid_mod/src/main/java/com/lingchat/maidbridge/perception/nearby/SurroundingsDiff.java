package com.lingchat.maidbridge.perception.nearby;

import com.google.gson.JsonObject;
import com.lingchat.maidbridge.perception.EventFrames;
import com.lingchat.maidbridge.perception.world.PerceptionDiff;
import com.lingchat.maidbridge.perception.nearby.Surroundings.Nearby;
import java.util.ArrayList;
import java.util.List;

/**
 * 身边环境的变化判定：上一帧 + 这一帧 → 该推给插件的事件。
 *
 * <p>纯逻辑，不碰游戏，能脱开 Minecraft 跑单测。与 {@link PerceptionDiff} 的分工一样：
 * 读世界在 {@link SurroundingsReader}，这里只判「该不该报」，中文措辞留给插件。
 * 帧的外形与取整见 {@link EventFrames}，脚下那类地的白名单见 {@link GroundKind}。
 *
 * <p>为什么判据要分档、要回滞、要连续两次采样才认：她自己在走动。原始距离与视线会一直抖，
 * 按数值比等于每五秒写一条旁白。所以这里只认档位跨档、只认稳定的变化，同类变化还有最短间隔。
 *
 * <p>本地这一层的**首帧永远不出事件**：刚接上或换了监控对象时只建基线（现状由全局那层的
 * {@code state_snapshot} 交代）。于是「重连不喷一屏」是语义上的保证，不靠「那一帧恰好没变化」。
 */
public final class SurroundingsDiff {
    /** 与主人的远近档：贴身 / 几步 / 有段距离 / 不在这个世界上。 */
    public static final double OWNER_CLOSE = 3.0;
    public static final double OWNER_NEAR = 8.0;
    public static final double OWNER_FAR = 16.0;
    /** 跨档余量：往近处跨档果断，往回退要多走两格，站在边界上不会来回跳。 */
    public static final double BAND_MARGIN = 2.0;
    /** 敌对生物多近算「靠过来了」；更远那一档只报「出现了」。 */
    public static final double THREAT_CLOSE = 6.0;
    /** 同类变化的最短间隔。 */
    public static final long PROXIMITY_COOLDOWN_TICKS = 100L;
    public static final long SIGHT_COOLDOWN_TICKS = 200L;
    public static final long SHELTER_COOLDOWN_TICKS = 400L;
    public static final long GROUND_COOLDOWN_TICKS = 600L;

    /**
     * 敌对生物的最短重报间隔：两分钟。
     *
     * <p>报过一次「有东西靠过来了」之后，这两分钟里新出现的、又走近的都不再另记一条——
     * 夜里围上来一群僵尸，玩家要的是知道有危险，不是每一只都来一句。这期间她身边站着谁
     * 另有交代：玩家开口时随消息带过去（插件侧），出事（挨打、血量告急、动作收尾）那条
     * 旁白后面也会缀一句眼前的活物。
     */
    public static final long THREAT_COOLDOWN_TICKS = 2400L;
    public static final long LIVING_COOLDOWN_TICKS = 1200L;
    /** 要连续这么多次采样都一样才认：走两步就跨档的那种不算。 */
    public static final int STABLE_SAMPLES = 2;

    private SurroundingsDiff() {
    }

    /**
     * 一个「要连续两次采样都一样才认」的取值，用来吃掉单帧抖动
     * （路过柱子时前后两帧正好一帧挡一帧不挡）。
     */
    private static final class StableValue {
        /** null 表示还没有基线。 */
        private String accepted;
        private String candidate;
        private int count;
        private long lastTick = Long.MIN_VALUE;

        void reset() {
            accepted = null;
            candidate = null;
            count = 0;
            lastTick = Long.MIN_VALUE;
        }

        /** 刚接上时用：认下现状但不报。 */
        void baseline(String value) {
            accepted = value;
            candidate = value;
            count = 0;
        }

        /** 记一次观测；返回该不该现在把新取值报出去。被冷静期压掉时不更新认可值，下次接着试。 */
        boolean observe(String value, long nowTick, long cooldownTicks) {
            if (value.equals(accepted)) {
                candidate = value;
                count = 0;
                return false;
            }
            if (value.equals(candidate)) {
                count++;
            } else {
                candidate = value;
                count = 1;
            }
            if (count < STABLE_SAMPLES || nowTick < lastTick + cooldownTicks) {
                return false;
            }
            accepted = value;
            count = 0;
            lastTick = nowTick;
            return true;
        }
    }

    /** 跨帧的记忆：三条要稳定才认的取值、威胁档、以及「最近提过哪几种活物」。 */
    public static final class State {
        private final StableValue proximity = new StableValue();
        private final StableValue sight = new StableValue();
        private final StableValue shelter = new StableValue();
        /** 已经报出去的威胁档；被冷静期压掉时不更新，好让下一次接着试。 */
        private String threatBand;
        private long lastThreatTick = Long.MIN_VALUE;
        /** 遮蔽与脚下共用：这两件事常常一起发生，不该一次说两句。 */
        private long lastSurfaceTick = Long.MIN_VALUE;
        /** 活物种类 → 上次为它写过旁白的 tick。 */
        private final KeyedCooldown living = new KeyedCooldown();

        /** 换了监控对象、重新接上、或女仆离开已加载区块时清掉，免得拿旧世界的状态去比。 */
        public void reset() {
            proximity.reset();
            sight.reset();
            shelter.reset();
            threatBand = null;
            lastThreatTick = Long.MIN_VALUE;
            lastSurfaceTick = Long.MIN_VALUE;
            living.reset();
        }
    }

    /** 这一帧该推的事件（可能一条都没有）。 */
    public static List<JsonObject> events(Surroundings prev, Surroundings next, State state, long nowTick) {
        List<JsonObject> frames = new ArrayList<>();

        // 刚接上或换了监控对象：只建基线，什么都不报
        if (prev == null || !prev.entityId().equals(next.entityId())) {
            state.reset();
            state.proximity.baseline(proximityBand(null, next));
            state.sight.baseline(sightValue(next));
            state.shelter.baseline(shelterValue(next));
            state.threatBand = threatBand(nearestHostile(next));
            return frames;
        }

        addProximity(frames, next, state, nowTick);
        addSight(frames, next, state, nowTick);
        addThreat(frames, next, state, nowTick);
        addLiving(frames, prev, next, state, nowTick);
        addShelter(frames, prev, next, state, nowTick);
        addGround(frames, prev, next, state, nowTick);
        return frames;
    }

    /** 主人离得远了还是近了。跨档要有回滞，免得站在边界上来回跳。 */
    private static void addProximity(
            List<JsonObject> frames, Surroundings next, State state, long nowTick) {
        String band = proximityBand(state.proximity.accepted, next);
        String before = state.proximity.accepted;
        if (band == null || !state.proximity.observe(band, nowTick, PROXIMITY_COOLDOWN_TICKS)) {
            return;
        }
        JsonObject frame = EventFrames.base("player_proximity_change", next.entityId());
        frame.addProperty("from_proximity", before);
        frame.addProperty("proximity", band);
        frame.addProperty("distance", EventFrames.round(next.owner().distance()));
        frames.add(frame);
    }

    /**
     * 主人进出视线。他不在这个世界上时不说这件事——「走出你的视线」与「去了别的世界」
     * 是两回事，后者由远近那条（{@code gone}）交代。
     */
    private static void addSight(List<JsonObject> frames, Surroundings next, State state, long nowTick) {
        if (proximityBand(state.proximity.accepted, next).equals("gone")) {
            return;
        }
        String value = sightValue(next);
        if (!state.sight.observe(value, nowTick, SIGHT_COOLDOWN_TICKS)) {
            return;
        }
        JsonObject frame = EventFrames.base("player_sight_change", next.entityId());
        frame.addProperty("visible", value.equals("visible"));
        frames.add(frame);
    }

    /**
     * 敌对生物靠近。只报「出现」与「由远转近」这两件事：走开与清空不报，
     * 否则打退一只僵尸要说一句「安全了」，来三只就要说三次。
     */
    private static void addThreat(List<JsonObject> frames, Surroundings next, State state, long nowTick) {
        Nearby threat = nearestHostile(next);
        String band = threatBand(threat);
        String before = state.threatBand;
        if (band.equals(before)) {
            return;
        }
        boolean escalated = band.equals("near") || (before != null && before.equals("none") && band.equals("far"));
        if (!escalated) {
            // 降级与清空：记下新档位但不说话
            state.threatBand = band;
            return;
        }
        if (nowTick < state.lastThreatTick + THREAT_COOLDOWN_TICKS) {
            return;
        }
        state.threatBand = band;
        state.lastThreatTick = nowTick;
        JsonObject frame = EventFrames.base("threat_near", next.entityId());
        frame.addProperty("type", threat.type());
        frame.addProperty("distance", EventFrames.round(threat.distance()));
        frame.addProperty("direction", threat.direction());
        frame.addProperty("visible", threat.visible());
        frames.add(frame);
    }

    /**
     * 身边来了别的活物。判据是「上一帧还没有」——一直待在那儿的牛不该反复提；
     * 同一种在冷静期内只提一次；一帧最多提一种，突然围上来一群也只说一句。
     */
    private static void addLiving(
            List<JsonObject> frames, Surroundings prev, Surroundings next, State state, long nowTick) {
        for (Nearby one : next.nearby()) {
            // 敌对生物有它自己那条，别重复
            if (one.category().equals("hostile") || hasType(prev, one.type())) {
                continue;
            }
            if (!state.living.allow(one.type(), nowTick, LIVING_COOLDOWN_TICKS)) {
                continue;
            }
            JsonObject frame = EventFrames.base("living_near", next.entityId());
            frame.addProperty("type", one.type());
            frame.addProperty("category", one.category());
            frame.addProperty("distance", EventFrames.round(one.distance()));
            frame.addProperty("direction", one.direction());
            frame.addProperty("visible", one.visible());
            frames.add(frame);
            return;
        }
    }

    private static boolean hasType(Surroundings s, String type) {
        for (Nearby one : s.nearby()) {
            if (one.type().equals(type)) {
                return true;
            }
        }
        return false;
    }

    /** 露天与遮蔽之间来回。没有天空的维度（下界、末地）不谈这件事。 */
    private static void addShelter(
            List<JsonObject> frames, Surroundings prev, Surroundings next, State state, long nowTick) {
        if (!prev.hasSky() || !next.hasSky()) {
            return;
        }
        if (!state.shelter.observe(shelterValue(next), nowTick, SHELTER_COOLDOWN_TICKS)) {
            return;
        }
        state.lastSurfaceTick = nowTick;
        JsonObject frame = EventFrames.base("shelter_change", next.entityId());
        frame.addProperty("sheltered", !next.skyAbove());
        frames.add(frame);
    }

    /** 脚下换了种地。只在「值得说」的几种之间变时报，走在石头路上不报。 */
    private static void addGround(
            List<JsonObject> frames, Surroundings prev, Surroundings next, State state, long nowTick) {
        String kind = GroundKind.of(next.feetBlock());
        if (kind.isEmpty() || kind.equals(GroundKind.of(prev.feetBlock()))) {
            return;
        }
        // 踩进水里、岩浆里、火里要立刻说，不被刚刚报过的遮蔽或脚下挡住
        if (!GroundKind.isUrgent(kind) && nowTick < state.lastSurfaceTick + GROUND_COOLDOWN_TICKS) {
            return;
        }
        state.lastSurfaceTick = nowTick;
        JsonObject frame = EventFrames.base("ground_change", next.entityId());
        frame.addProperty("block", next.feetBlock());
        frame.addProperty("kind", kind);
        frames.add(frame);
    }

    private static Nearby nearestHostile(Surroundings s) {
        if (s == null) {
            return null;
        }
        Nearby nearest = null;
        for (Nearby one : s.nearby()) {
            if (one.category().equals("hostile") && (nearest == null || one.distance() < nearest.distance())) {
                nearest = one;
            }
        }
        return nearest;
    }

    private static String threatBand(Nearby threat) {
        if (threat == null) {
            return "none";
        }
        return threat.distance() <= THREAT_CLOSE ? "near" : "far";
    }

    private static String sightValue(Surroundings s) {
        return s.owner().sameDimension() && s.owner().distance() >= 0 && s.owner().visible() ? "visible" : "hidden";
    }

    private static String shelterValue(Surroundings s) {
        return s.skyAbove() ? "open" : "sheltered";
    }

    /** 远近档。{@code before} 是已认可的档位，用它做回滞；没有基线时按严格阈值判。 */
    private static String proximityBand(String before, Surroundings s) {
        if (!s.owner().sameDimension() || s.owner().distance() < 0) {
            return "gone";
        }
        double distance = s.owner().distance();
        if (before == null) {
            return bandOf(distance, 0.0);
        }
        // 旧档位在放宽余量后仍然成立就不动；否则按严格阈值判新档（往近处跨档是果断的）
        if (bandOf(distance, BAND_MARGIN).equals(before)) {
            return before;
        }
        return bandOf(distance, 0.0);
    }

    private static String bandOf(double distance, double margin) {
        if (distance <= OWNER_CLOSE + margin) {
            return "close";
        }
        if (distance <= OWNER_NEAR + margin) {
            return "near";
        }
        if (distance <= OWNER_FAR + margin) {
            return "far";
        }
        return "gone";
    }
}
