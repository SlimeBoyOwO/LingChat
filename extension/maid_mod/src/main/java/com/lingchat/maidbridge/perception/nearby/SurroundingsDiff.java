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
    /** 同类变化的最短间隔。环境类一律一分钟：这些事变得比旁白能说的还快。 */
    public static final long PROXIMITY_COOLDOWN_TICKS = 100L;
    public static final long SIGHT_COOLDOWN_TICKS = 200L;
    public static final long SHELTER_COOLDOWN_TICKS = 1200L;
    public static final long GROUND_COOLDOWN_TICKS = 1200L;
    /** 进出结构的最短重报间隔：在村子边上走来走去不该反复说。 */
    public static final long STRUCTURE_COOLDOWN_TICKS = 1200L;
    /** 光线跨档的最短重报间隔：洞里来回走、点灭火把不该反复说。 */
    public static final long LIGHT_COOLDOWN_TICKS = 1200L;
    /** 泡进液体与埋头游泳的最短重报间隔，与环境类一致：一分钟。 */
    public static final long LIQUID_COOLDOWN_TICKS = 1200L;
    /** 光照分档阈值：0-3 很黑，4-7 昏暗，8 以上明亮。 */
    public static final int LIGHT_DARK_MAX = 3;
    public static final int LIGHT_DIM_MAX = 7;

    /**
     * 敌对生物的最短重报间隔：两分钟。
     *
     * <p>报过一次「有东西靠过来了」之后，这两分钟里新出现的、又走近的都不再另记一条——
     * 夜里围上来一群僵尸，玩家要的是知道有危险，不是每一只都来一句。这期间她身边站着谁
     * 另有交代：出事那条旁白（挨打、血量告急、动作收尾）后面会缀一句眼前的活物（插件侧）。
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
        private final StableValue light = new StableValue();
        /** 已经报出去的威胁档；被冷静期压掉时不更新，好让下一次接着试。 */
        private String threatBand;
        private long lastThreatTick = Long.MIN_VALUE;
        /** 遮蔽与脚下共用：这两件事常常一起发生，不该一次说两句。 */
        private long lastSurfaceTick = Long.MIN_VALUE;
        /** 活物种类 → 上次为它写过旁白的 tick。 */
        private final KeyedCooldown living = new KeyedCooldown();
        /** 上次报结构变化的时间：在村子边上反复进出不该刷屏。 */
        private long lastStructureTick = Long.MIN_VALUE;
        /** 已报出的液体状态：空串 = 没泡着，否则是液体 id，埋头时再缀 {@code |swim}。 */
        private String liquid = "";
        private long lastLiquidTick = Long.MIN_VALUE;

        /** 换了监控对象、重新接上、或女仆离开已加载区块时清掉，免得拿旧世界的状态去比。 */
        public void reset() {
            proximity.reset();
            sight.reset();
            shelter.reset();
            light.reset();
            threatBand = null;
            lastThreatTick = Long.MIN_VALUE;
            lastSurfaceTick = Long.MIN_VALUE;
            living.reset();
            lastStructureTick = Long.MIN_VALUE;
            liquid = "";
            lastLiquidTick = Long.MIN_VALUE;
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
            state.light.baseline(lightBand(next.light()));
            state.threatBand = threatBand(nearestHostile(next));
            state.liquid = liquidValue(next);
            return frames;
        }

        addProximity(frames, next, state, nowTick);
        addSight(frames, next, state, nowTick);
        addThreat(frames, next, state, nowTick);
        addLiving(frames, prev, next, state, nowTick);
        addShelter(frames, prev, next, state, nowTick);
        addLight(frames, next, state, nowTick);
        addVehicle(frames, prev, next);
        addStructure(frames, prev, next, state, nowTick);
        addLiquid(frames, prev, next, state, nowTick);
        addGround(frames, prev, next, state, nowTick);
        addPosture(frames, prev, next);
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

    /** 头顶换了档：露天 / 半遮 / 屋内。没有天空的维度（下界、末地）不谈这件事。 */
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
        frame.addProperty("overhead", next.overhead().kind());
        frame.addProperty("overhead_cover", next.overhead().cover());
        frame.addProperty("overhead_band", next.overhead().band());
        // 同一段时间里先进树荫又走出来时，两句并排记下来读着像来回折腾。插件按这个
        // tick 把同键的合成一条，只留最后那次（数值大的胜出）。
        frame.addProperty("tick", nowTick);
        frames.add(frame);
    }

    /**
     * 光线跨了一档：很黑 / 昏暗 / 明亮。进洞、天黑、点起火把都会走到这里。
     *
     * <p>只报跨档，不报数值；和遮蔽一样要连续两次采样才认，洞口来回走不会反复说。
     */
    private static void addLight(List<JsonObject> frames, Surroundings next, State state, long nowTick) {
        String band = lightBand(next.light());
        if (!state.light.observe(band, nowTick, LIGHT_COOLDOWN_TICKS)) {
            return;
        }
        JsonObject frame = EventFrames.base("light_change", next.entityId());
        frame.addProperty("band", band);
        frames.add(frame);
    }

    /** 光照等级 → 档位；阈值与插件的旁白说法一一对应。 */
    public static String lightBand(int light) {
        if (light <= LIGHT_DARK_MAX) {
            return "dark";
        }
        return light <= LIGHT_DIM_MAX ? "dim" : "bright";
    }

    /** 上去或下来了：船、矿车这类载具。坐在里面的时候脚下没有「地」可说。 */
    private static void addVehicle(List<JsonObject> frames, Surroundings prev, Surroundings next) {
        if (prev.vehicle().equals(next.vehicle())) {
            return;
        }
        JsonObject frame = EventFrames.base("vehicle_change", next.entityId());
        frame.addProperty("vehicle", next.vehicle());
        frame.addProperty("from_vehicle", prev.vehicle());
        frames.add(frame);
    }

    /** 走进或离开一处结构（村庄、矿井这类）。只在真的换了结构时报，且有冷静期。 */
    private static void addStructure(
            List<JsonObject> frames, Surroundings prev, Surroundings next, State state, long nowTick) {
        if (prev.structure().equals(next.structure())) {
            return;
        }
        if (nowTick < state.lastStructureTick + STRUCTURE_COOLDOWN_TICKS) {
            return;
        }
        state.lastStructureTick = nowTick;
        JsonObject frame = EventFrames.base("structure_change", next.entityId());
        frame.addProperty("structure", next.structure());
        frame.addProperty("from_structure", prev.structure());
        frames.add(frame);
    }

    /**
     * 泡进液体里：水、岩浆或模组液体，以及头也浸进去的游泳。
     *
     * <p>与脚下那层分开：那里的白名单说的是「踩着什么样的地」，泡在液体里是另一件事——
     * 跨进水里时脚下还是河床，所以她这一格有没有液体才是判据。埋头与抬头也算一次变化。
     * 与环境类一样一分钟最多报一次，浅滩里踩进踩出不会一句接一句。
     */
    private static void addLiquid(
            List<JsonObject> frames, Surroundings prev, Surroundings next, State state, long nowTick) {
        // 坐在船里、矿车里时她在水面之上，不是自己泡着：上下船那一下不当作进水出水
        if (!next.vehicle().isEmpty() || !prev.vehicle().isEmpty()) {
            return;
        }
        String value = liquidValue(next);
        if (value.equals(state.liquid)) {
            return;
        }
        state.liquid = value;
        // 冷静期内换了液体（浅滩里踩进踩出、岸边被浪打一下）不补报
        if (nowTick < state.lastLiquidTick + LIQUID_COOLDOWN_TICKS) {
            return;
        }
        state.lastLiquidTick = nowTick;
        JsonObject frame = EventFrames.base("liquid_change", next.entityId());
        frame.addProperty("liquid", next.liquid());
        frame.addProperty("from_liquid", prev.liquid());
        frame.addProperty("swimming", next.submerged());
        frames.add(frame);
    }

    /** 液体状态的比较口径：没泡着为空串，泡着是液体 id，埋头再缀一个标记。 */
    private static String liquidValue(Surroundings s) {
        if (s.liquid().isEmpty()) {
            return "";
        }
        return s.submerged() ? s.liquid() + "|swim" : s.liquid();
    }

    /** 脚下换了种地。只在「值得说」的几种之间变时报，走在石头路上不报。 */
    private static void addGround(
            List<JsonObject> frames, Surroundings prev, Surroundings next, State state, long nowTick) {
        // 坐在载具里脚下没有地：上下车那一下不当作「踩进了水里/沙地上」
        if (!next.vehicle().isEmpty() || !prev.vehicle().isEmpty()) {
            return;
        }
        // 泡在液体里的那一步交给 liquid_change，脚下那块地这时说不说都一样
        if (!next.liquid().isEmpty() || !prev.liquid().isEmpty()) {
            return;
        }
        String kind = GroundKind.of(next.feetBlock());
        if (kind.isEmpty() || kind.equals(GroundKind.of(prev.feetBlock()))) {
            return;
        }
        // 踩进岩浆、火里要立刻说，不被刚刚报过的遮蔽或脚下挡住；水只算换了种地
        if (!GroundKind.isUrgent(kind) && nowTick < state.lastSurfaceTick + GROUND_COOLDOWN_TICKS) {
            return;
        }
        state.lastSurfaceTick = nowTick;
        JsonObject frame = EventFrames.base("ground_change", next.entityId());
        frame.addProperty("block", next.feetBlock());
        frame.addProperty("kind", kind);
        frames.add(frame);
    }

    /**
     * 她换了姿势：坐下、站起、就地躺下、上床睡着、醒过来。
     *
     * <p>这是离散动作，不是需要回滞的连续量：变了就报，不设冷静期。首帧仍由外层统一
     * 建基线，所以重连不会把当前姿势再念一遍。
     */
    private static void addPosture(
            List<JsonObject> frames, Surroundings prev, Surroundings next) {
        // 昏倒与醒来由专门事件交代；这里不再说成「躺着/睡着」
        if ("unconscious".equals(prev.posture()) || "unconscious".equals(next.posture())) {
            return;
        }
        if (prev.posture().equals(next.posture())) {
            return;
        }
        JsonObject frame = EventFrames.base("posture_change", next.entityId());
        frame.addProperty("posture", next.posture());
        frame.addProperty("from_posture", prev.posture());
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
        String kind = s.overhead().kind();
        // 老数据或没扫出结果时退回布尔，三档字段照样能空着走
        return kind.isEmpty() ? (s.skyAbove() ? "open" : "sheltered") : kind;
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
