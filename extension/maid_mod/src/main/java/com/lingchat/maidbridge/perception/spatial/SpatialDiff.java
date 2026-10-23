package com.lingchat.maidbridge.perception.spatial;

import com.google.gson.JsonObject;
import com.lingchat.maidbridge.perception.EventFrames;
import com.lingchat.maidbridge.perception.nearby.KeyedCooldown;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

/**
 * 危险方块的空间差分：这一帧比她上一帧多看见了什么。
 *
 * <p>纯逻辑，不碰游戏，能脱开 Minecraft 跑单测。判据是「这一种危险原来不在她眼里」，
 * 而不是「离她几格」——沿着岩浆湖走，最近的那一格每一步都在变，按距离比会一路报下去。
 * 同一种离开视野又回来另有冷静期，走过树干挡一下不会反复说。
 *
 * <p>她脚下那一列由 {@code ground_change} 交代，这里只补「边上」的那一类。首帧只建基线，
 * 与身边那一层的约定一致：刚接上或换了监控对象时不报旧账。
 */
public final class SpatialDiff {
    /** 同一种危险离开视野后又回来的最短重报间隔：一分钟。 */
    public static final long HAZARD_COOLDOWN_TICKS = 1200L;

    private SpatialDiff() {
    }

    /** 跨帧的记忆：每种危险上次报是什么时候。上一帧看见了什么由调用方传入的 prev 交代。 */
    public static final class State {
        private final KeyedCooldown reported = new KeyedCooldown();

        public void reset() {
            reported.reset();
        }
    }

    /** 这一帧该推的事件（最多一条）。 */
    public static List<JsonObject> events(
            SpatialView prev, SpatialView next, State state, long nowTick) {
        List<JsonObject> frames = new ArrayList<>();
        // 首帧与换监控对象：只认下现状，不报
        if (prev == null || !prev.entityId().equals(next.entityId())) {
            state.reset();
            return frames;
        }

        Set<String> before = ids(prev);
        List<SpatialView.Hazard> fresh = new ArrayList<>();
        for (SpatialView.Hazard hazard : next.hazards()) {
            if (!before.contains(hazard.id())) {
                fresh.add(hazard);
            }
        }
        fresh.sort(Comparator.comparingDouble(SpatialView.Hazard::distance));
        SpatialView.Hazard picked = null;
        for (SpatialView.Hazard hazard : fresh) {
            // 最近的先试；这一种还在冷静期里就退而求其次，别把另一种新危险也一起压掉
            if (state.reported.allow(hazard.id(), nowTick, HAZARD_COOLDOWN_TICKS)) {
                picked = hazard;
                break;
            }
        }
        if (picked == null) {
            return frames;
        }
        JsonObject frame = EventFrames.base("hazard_near", next.entityId());
        frame.addProperty("block", picked.id());
        frame.addProperty("distance", EventFrames.round(picked.distance()));
        frame.addProperty("direction", picked.direction());
        frames.add(frame);
        return frames;
    }

    private static Set<String> ids(SpatialView view) {
        Set<String> ids = new LinkedHashSet<>();
        for (SpatialView.Hazard hazard : view.hazards()) {
            ids.add(hazard.id());
        }
        return ids;
    }
}
