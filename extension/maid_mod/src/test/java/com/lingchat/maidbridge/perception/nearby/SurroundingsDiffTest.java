package com.lingchat.maidbridge.perception.nearby;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

/** 身边环境判定的单测：重点在「不该报」的那一半。 */
class SurroundingsDiffTest {
    private static final String MAID = "maid-1";

    @Test
    void firstFrameEmitsNothing() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        assertTrue(
                SurroundingsDiff.events(null, plain(), state, 100L).isEmpty(),
                "刚接上只建基线，不推事件");
    }

    @Test
    void switchingMonitoredMaidResetsBaseline() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        SurroundingsDiff.events(null, plain(), state, 100L);
        Surroundings otherMaid = withId(plain(), "maid-2");
        assertTrue(
                SurroundingsDiff.events(plain(), otherMaid, state, 200L).isEmpty(),
                "换了监控对象同样只建基线");
    }

    @Test
    void ownerWalkingAwayNeedsTwoAgreeingFrames() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        Surroundings close = frame(owner(2.0, true, true), List.of());
        SurroundingsDiff.events(null, close, state, 100L);

        Surroundings far = frame(owner(12.0, true, true), List.of());
        assertTrue(diff(close, far, state, 200L).isEmpty(), "跨档第一次只是待确认");

        List<JsonObject> frames = diff(far, far, state, 300L);
        assertEquals(List.of("player_proximity_change"), types(frames), "连续两次同档才认");
        assertEquals("close", frames.get(0).get("from_proximity").getAsString(), "要说清是从哪一档过来的");
        assertEquals("far", frames.get(0).get("proximity").getAsString());
    }

    @Test
    void ownerJitterAtTheBoundaryIsIgnored() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        SurroundingsDiff.events(null, frame(owner(2.9, true, true), List.of()), state, 100L);

        // 在「贴身」那条阈值上反复蹭：有回滞就不该出声
        double[] wobble = {3.2, 2.8, 3.4, 2.6};
        List<JsonObject> frames = new ArrayList<>();
        for (int i = 0; i < wobble.length; i++) {
            Surroundings one = frame(owner(wobble[i], true, true), List.of());
            frames.addAll(diff(one, one, state, 200L + i * 100L));
        }
        assertTrue(frames.isEmpty(), "站在档位边界上来回蹭不该报");
    }

    @Test
    void sightFlipNeedsTwoAgreeingFrames() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        Surroundings seen = frame(owner(6.0, true, true), List.of());
        SurroundingsDiff.events(null, seen, state, 100L);

        Surroundings blocked = frame(owner(6.0, false, true), List.of());
        assertTrue(diff(seen, blocked, state, 200L).isEmpty(), "一次翻转只是待确认");

        List<JsonObject> frames = diff(blocked, blocked, state, 300L);
        assertEquals(List.of("player_sight_change"), types(frames));
        assertFalse(frames.get(0).get("visible").getAsBoolean(), "被挡住了就是看不见");
    }

    @Test
    void ownerInAnotherDimensionIsOnlyReportedAsGone() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        Surroundings here = frame(owner(6.0, true, true), List.of());
        SurroundingsDiff.events(null, here, state, 100L);

        Surroundings elsewhere = frame(Surroundings.Owner.absent(), List.of());
        assertTrue(diff(here, elsewhere, state, 200L).isEmpty(), "跨维度第一次也是待确认");

        List<JsonObject> frames = diff(elsewhere, elsewhere, state, 300L);
        assertEquals(List.of("player_proximity_change"), types(frames), "去了别的世界只报远近那条");
        assertEquals("gone", frames.get(0).get("proximity").getAsString());
    }

    @Test
    void threatAppearingIsReported() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        SurroundingsDiff.events(null, plain(), state, 100L);

        Surroundings zombie = withNearby(plain(), mob("minecraft:zombie", "hostile", 10.0));
        assertEquals(
                List.of("threat_near"),
                types(diff(plain(), zombie, state, 200L)),
                "出现了敌对生物要报");
    }

    @Test
    void threatEscalatingToCloseWaitsOutTheTwoMinuteDebounce() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        SurroundingsDiff.events(null, plain(), state, 100L);
        Surroundings far = withNearby(plain(), mob("minecraft:zombie", "hostile", 10.0));
        diff(plain(), far, state, 200L);

        Surroundings close = withNearby(plain(), mob("minecraft:zombie", "hostile", 3.0));
        assertTrue(diff(far, close, state, 300L).isEmpty(), "两分钟之内别的怪物不再另记一条");
        assertTrue(
                diff(close, close, state, 200L + SurroundingsDiff.THREAT_COOLDOWN_TICKS - 100L).isEmpty(),
                "防抖还没过");
        assertEquals(
                List.of("threat_near"),
                types(diff(close, close, state, 200L + SurroundingsDiff.THREAT_COOLDOWN_TICKS)),
                "压住不等于丢掉：防抖过了要补报");
    }

    @Test
    void threatLeavingIsSilent() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        SurroundingsDiff.events(null, plain(), state, 100L);
        Surroundings zombie = withNearby(plain(), mob("minecraft:zombie", "hostile", 3.0));
        diff(plain(), zombie, state, 200L);

        assertTrue(diff(zombie, plain(), state, 300L).isEmpty(), "威胁走开不报「安全了」");
        assertTrue(diff(plain(), plain(), state, 400L).isEmpty());
    }

    @Test
    void sameCompanyIsReportedOnlyOnArrival() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        SurroundingsDiff.events(null, plain(), state, 100L);
        Surroundings cow = withNearby(plain(), mob("minecraft:cow", "living", 4.0));

        assertEquals(List.of("living_near"), types(diff(plain(), cow, state, 200L)), "刚来要提一句");
        assertTrue(diff(cow, cow, state, 300L).isEmpty(), "一直在的牛不该反复提");
        assertTrue(diff(cow, plain(), state, 1000L).isEmpty(), "走了不报");
        assertTrue(diff(plain(), cow, state, 1100L).isEmpty(), "刚走开又回来，冷静期内不重复");
        assertTrue(diff(cow, cow, state, 2000L).isEmpty(), "待在原地就不提");
        assertEquals(
                List.of("living_near"),
                types(diff(plain(), cow, state, 2200L)),
                "过了冷静期、又是刚回来，可以再提一次");
        assertTrue(diff(cow, cow, state, 2300L).isEmpty(), "之后就待在原地，不再提");
    }

    @Test
    void aHostileIsNotCompany() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        SurroundingsDiff.events(null, plain(), state, 100L);
        Surroundings zombie = withNearby(plain(), mob("minecraft:zombie", "hostile", 5.0));
        assertEquals(
                List.of("threat_near"),
                types(diff(plain(), zombie, state, 200L)),
                "僵尸只走威胁那条，不该同时算成「别的活物」");
    }

    @Test
    void atMostOneCompanyPerFrame() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        SurroundingsDiff.events(null, plain(), state, 100L);
        Surroundings crowd = withNearby(
                plain(),
                mob("minecraft:cow", "living", 3.0),
                mob("minecraft:villager", "living", 4.0),
                mob("minecraft:sheep", "living", 5.0));
        assertEquals(
                List.of("living_near"),
                types(diff(plain(), crowd, state, 200L)),
                "突然一群只提一句，别一次刷三行");
    }

    @Test
    void ordinaryGroundChangeIsSilent() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        Surroundings grass = withFeet(plain(), "minecraft:grass_block");
        SurroundingsDiff.events(null, grass, state, 100L);

        Surroundings stone = withFeet(plain(), "minecraft:stone");
        assertTrue(diff(grass, stone, state, 200L).isEmpty(), "草地换石头不值得说");
        assertTrue(diff(stone, withFeet(plain(), "minecraft:dirt"), state, 300L).isEmpty(), "石头换泥土也不说");
    }

    @Test
    void steppingIntoWaterIsReportedEvenInsideTheCooldown() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        Surroundings grass = withFeet(plain(), "minecraft:grass_block");
        SurroundingsDiff.events(null, grass, state, 100L);

        Surroundings snow = withFeet(plain(), "minecraft:snow_block");
        assertEquals(List.of("ground_change"), types(diff(grass, snow, state, 200L)), "上了雪要提一句");

        Surroundings water = withFeet(plain(), "minecraft:water");
        List<JsonObject> frames = diff(snow, water, state, 300L);
        assertEquals(List.of("ground_change"), types(frames), "踩进水里不受冷静期挡");
        assertEquals("water", frames.get(0).get("kind").getAsString());
    }

    @Test
    void shelterFlipIsReportedAfterTwoFrames() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        Surroundings outside = plain();
        SurroundingsDiff.events(null, outside, state, 100L);

        Surroundings inside = withSky(outside, false);
        assertTrue(diff(outside, inside, state, 200L).isEmpty(), "一次进洞只是待确认");

        List<JsonObject> frames = diff(inside, inside, state, 300L);
        assertEquals(List.of("shelter_change"), types(frames));
        assertTrue(frames.get(0).get("sheltered").getAsBoolean(), "头顶看不到天就是进了遮蔽处");
    }

    @Test
    void shelterIsSkippedWithoutSky() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        Surroundings netherClosed = withHasSky(withSky(plain(), false), false);
        SurroundingsDiff.events(null, netherClosed, state, 100L);

        Surroundings netherOpen = withHasSky(withSky(plain(), true), false);
        assertTrue(diff(netherClosed, netherOpen, state, 200L).isEmpty(), "下界不谈露天");
        assertTrue(diff(netherOpen, netherOpen, state, 300L).isEmpty(), "下界不谈露天");
    }

    @Test
    void everyFrameCarriesTypeAndMaidId() {
        SurroundingsDiff.State state = new SurroundingsDiff.State();
        SurroundingsDiff.events(null, plain(), state, 100L);
        List<JsonObject> frames = new ArrayList<>();
        frames.addAll(diff(plain(), withNearby(plain(), mob("minecraft:cow", "living", 3.0)), state, 200L));
        frames.addAll(diff(plain(), withFeet(plain(), "minecraft:water"), state, 300L));
        frames.addAll(diff(plain(), frame(owner(2.0, false, true), List.of()), state, 400L));
        assertFalse(frames.isEmpty(), "这几帧总得产生点事件");
        for (JsonObject frame : frames) {
            assertEquals(MAID, frame.get("maid_id").getAsString(), "每一帧都要带 maid_id");
            assertFalse(frame.get("event_type").getAsString().isEmpty(), "每一帧都要带 event_type");
        }
    }

    @Test
    void directionCoversEightWays() {
        assertEquals("front", Surroundings.direction(0.0, 0.0, 1.0), "面朝 +Z 时正前方就是 +Z");
        assertEquals("back", Surroundings.direction(0.0, 0.0, -1.0), "背后");
        assertEquals("right", Surroundings.direction(0.0, -1.0, 0.0), "面朝 +Z 时右手边是 -X");
        assertEquals("left", Surroundings.direction(0.0, 1.0, 0.0), "左手边");
        assertEquals("front_right", Surroundings.direction(0.0, -0.5, 0.866), "右前");
        assertEquals("back_left", Surroundings.direction(0.0, 0.7, -0.7), "左后");
        assertEquals("front", Surroundings.direction(90.0, -1.0, 0.0), "转过九十度，原来的右方成了正前");
    }

    @Test
    void nearbyListIsSortedAndCapped() {
        List<Surroundings.Nearby> raw = new ArrayList<>();
        for (int i = 0; i < 10; i++) {
            raw.add(mob("mob-" + i, "living", 10.0 - i));
        }
        List<Surroundings.Nearby> trimmed = Surroundings.trimmed(raw);
        assertEquals(Surroundings.NEARBY_LIMIT, trimmed.size(), "最多留三只");
        assertEquals("mob-9", trimmed.get(0).type(), "最近的排最前");
        assertEquals("mob-7", trimmed.get(2).type(), "第四只往后就切掉");
    }

    @Test
    void invisibleOnlyCountsWhenClose() {
        Surroundings.Profile profile = Surroundings.Profile.of(12.0, 6.0);
        List<Surroundings.Nearby> raw = new ArrayList<>();
        raw.add(new Surroundings.Nearby("far-visible", "living", 11.0, "front", true));
        raw.add(new Surroundings.Nearby("far-hidden", "living", 11.0, "front", false));
        raw.add(new Surroundings.Nearby("close-hidden", "living", 5.0, "front", false));
        raw.add(new Surroundings.Nearby("edge-visible", "living", 12.0, "front", true));

        List<Surroundings.Nearby> kept = Surroundings.eligible(raw, profile);
        assertEquals(3, kept.size(), "看得见 12 格、看不见 6 格");
        assertEquals("far-visible", kept.get(0).type());
        assertEquals("close-hidden", kept.get(1).type());
        assertEquals("edge-visible", kept.get(2).type());
    }

    @Test
    void responseCarriesRegistryIdsOnly() {
        JsonObject frame = Surroundings.response(plain());
        assertEquals("maid_look_around_result", frame.get("type").getAsString());
        JsonObject data = frame.getAsJsonObject("data");
        assertEquals(MAID, data.get("maid_id").getAsString());
        assertFalse(
                data.has("x") || data.has("y") || data.has("z"),
                "协议里不出现坐标：接口层面没有接受坐标的入口，也不外发她站在哪");
    }

    // ─── 造数据 ───

    private static List<JsonObject> diff(
            Surroundings prev, Surroundings next, SurroundingsDiff.State state, long nowTick) {
        return SurroundingsDiff.events(prev, next, state, nowTick);
    }

    private static List<String> types(List<JsonObject> frames) {
        List<String> out = new ArrayList<>();
        for (JsonObject frame : frames) {
            out.add(frame.get("event_type").getAsString());
        }
        return out;
    }

    private static Surroundings plain() {
        return frame(owner(10.0, true, true), List.of());
    }

    private static Surroundings frame(Surroundings.Owner owner, List<Surroundings.Nearby> nearby) {
        return new Surroundings(
                MAID,
                "minecraft:overworld",
                Surroundings.Gaze.none(),
                "minecraft:grass_block",
                "",
                true,
                true,
                15,
                owner,
                nearby,
                false);
    }

    private static Surroundings.Owner owner(double distance, boolean visible, boolean sameDimension) {
        return new Surroundings.Owner(sameDimension ? "NoiQ" : "", distance, visible, sameDimension);
    }

    private static Surroundings.Nearby mob(String type, String category, double distance) {
        return new Surroundings.Nearby(type, category, distance, "front", true);
    }

    private static Surroundings withId(Surroundings s, String entityId) {
        return new Surroundings(
                entityId, s.dimension(), s.gaze(), s.feetBlock(), s.footing(), s.skyAbove(), s.hasSky(),
                s.light(), s.owner(), s.nearby(), s.nearbyTruncated());
    }

    private static Surroundings withNearby(Surroundings s, Surroundings.Nearby... nearby) {
        return new Surroundings(
                s.entityId(), s.dimension(), s.gaze(), s.feetBlock(), s.footing(), s.skyAbove(), s.hasSky(),
                s.light(), s.owner(), List.of(nearby), s.nearbyTruncated());
    }

    private static Surroundings withFeet(Surroundings s, String block) {
        return new Surroundings(
                s.entityId(), s.dimension(), s.gaze(), block, s.footing(), s.skyAbove(), s.hasSky(),
                s.light(), s.owner(), s.nearby(), s.nearbyTruncated());
    }

    private static Surroundings withSky(Surroundings s, boolean skyAbove) {
        return new Surroundings(
                s.entityId(), s.dimension(), s.gaze(), s.feetBlock(), s.footing(), skyAbove, s.hasSky(),
                s.light(), s.owner(), s.nearby(), s.nearbyTruncated());
    }

    private static Surroundings withHasSky(Surroundings s, boolean hasSky) {
        return new Surroundings(
                s.entityId(), s.dimension(), s.gaze(), s.feetBlock(), s.footing(), s.skyAbove(), hasSky,
                s.light(), s.owner(), s.nearby(), s.nearbyTruncated());
    }
}
