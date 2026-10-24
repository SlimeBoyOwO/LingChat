package com.lingchat.maidbridge.perception.spatial;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.google.gson.JsonObject;
import java.util.List;
import org.junit.jupiter.api.Test;

/** 危险方块的空间差分：只报新出现在眼里的，同一种不刷屏。 */
class SpatialDiffTest {
    private static final String MAID = "maid-1";
    private static final String LAVA = "minecraft:lava";
    private static final String FIRE = "minecraft:fire";

    private static SpatialView view(SpatialView.Hazard... hazards) {
        return new SpatialView(MAID, List.of(hazards));
    }

    private static SpatialView.Hazard hazard(String id, double distance) {
        return new SpatialView.Hazard(id, distance, "front");
    }

    @Test
    void firstFrameOnlyBuildsBaseline() {
        SpatialDiff.State state = new SpatialDiff.State();
        assertTrue(SpatialDiff.events(null, view(hazard(LAVA, 6.0)), state, 100L).isEmpty());
        // 基线里的岩浆不该在下一帧再报一次
        assertTrue(SpatialDiff.events(view(hazard(LAVA, 6.0)), view(hazard(LAVA, 6.0)), state, 200L)
                .isEmpty());
    }

    @Test
    void newlySeenHazardIsReported() {
        SpatialDiff.State state = new SpatialDiff.State();
        List<JsonObject> frames = SpatialDiff.events(view(), view(hazard(LAVA, 6.0)), state, 100L);
        assertEquals(1, frames.size());
        JsonObject frame = frames.get(0);
        assertEquals("hazard_near", frame.get("event_type").getAsString());
        assertEquals(MAID, frame.get("maid_id").getAsString());
        assertEquals(LAVA, frame.get("block").getAsString());
        assertEquals(6.0, frame.get("distance").getAsDouble(), 1.0E-6);
        assertEquals("front", frame.get("direction").getAsString());
    }

    @Test
    void sameHazardDoesNotRepeatWhileInSight() {
        SpatialDiff.State state = new SpatialDiff.State();
        assertEquals(1, SpatialDiff.events(view(), view(hazard(LAVA, 6.0)), state, 100L).size());
        // 沿着岩浆湖走：最近的那一格一直在换，但种类没变
        assertTrue(SpatialDiff.events(view(hazard(LAVA, 6.0)), view(hazard(LAVA, 3.0)), state, 200L)
                .isEmpty());
    }

    @Test
    void leavingAndReturningWaitsForCooldown() {
        SpatialDiff.State state = new SpatialDiff.State();
        assertEquals(1, SpatialDiff.events(view(), view(hazard(LAVA, 6.0)), state, 100L).size());
        assertTrue(SpatialDiff.events(view(hazard(LAVA, 6.0)), view(), state, 200L).isEmpty());
        // 树缝里又露出来：一分钟之内不再说
        assertTrue(SpatialDiff.events(view(), view(hazard(LAVA, 6.0)), state, 300L).isEmpty());
        long later = 100L + SpatialDiff.HAZARD_COOLDOWN_TICKS;
        assertEquals(1, SpatialDiff.events(view(), view(hazard(LAVA, 6.0)), state, later).size());
    }

    @Test
    void nearestNewHazardIsChosen() {
        SpatialDiff.State state = new SpatialDiff.State();
        List<JsonObject> frames =
                SpatialDiff.events(view(), view(hazard(LAVA, 7.0), hazard(FIRE, 3.0)), state, 100L);
        assertEquals(1, frames.size());
        assertEquals(FIRE, frames.get(0).get("block").getAsString());
    }

    @Test
    void cooledDownHazardDoesNotHideAnotherKind() {
        SpatialDiff.State state = new SpatialDiff.State();
        assertEquals(1, SpatialDiff.events(view(), view(hazard(LAVA, 6.0)), state, 100L).size());
        // 岩浆还在冷却期里，火是新的：不能因为最近的岩浆被压掉就两条都不说
        List<JsonObject> frames =
                SpatialDiff.events(view(), view(hazard(LAVA, 2.0), hazard(FIRE, 5.0)), state, 200L);
        assertEquals(1, frames.size());
        assertEquals(FIRE, frames.get(0).get("block").getAsString());
    }

    @Test
    void otherMaidRebuildsTheBaseline() {
        SpatialDiff.State state = new SpatialDiff.State();
        assertEquals(1, SpatialDiff.events(view(), view(hazard(LAVA, 6.0)), state, 100L).size());
        SpatialView other = new SpatialView("maid-2", List.of(hazard(LAVA, 6.0)));
        assertTrue(SpatialDiff.events(view(), other, state, 200L).isEmpty());
    }
}
