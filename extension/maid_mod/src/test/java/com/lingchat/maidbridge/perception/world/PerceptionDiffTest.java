package com.lingchat.maidbridge.perception.world;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import com.google.gson.JsonObject;
import java.util.List;
import org.junit.jupiter.api.Test;

/**
 * 变化判定：这部分是纯逻辑，能脱开游戏跑。
 *
 * <p>要守住的是三件容易出错的事：重新接上 / 换监控对象时必须先交代现状；换群系要有
 * 冷静期（否则鞘翅赶路会把台词表刷满）；血量告警只在跌破那一下报，且要能复位。
 */
class PerceptionDiffTest {
    private static final double LOW_RATIO = 0.3;

    private final PerceptionDiff.State state = new PerceptionDiff.State();

    private static PerceptionSnapshot snapshot(String biome) {
        return snapshot("minecraft:overworld", biome, 20.0, 20.0);
    }

    private static PerceptionSnapshot snapshot(
            String dimension, String biome, double playerHealth, double maidHealth) {
        return new PerceptionSnapshot(
                "maid-1",
                dimension,
                biome,
                true,
                false,
                false,
                1000L,
                "史蒂夫",
                playerHealth,
                20.0,
                "小狐",
                maidHealth,
                20.0,
                "standing");
    }

    private List<JsonObject> diff(PerceptionSnapshot prev, PerceptionSnapshot next, long tick) {
        return PerceptionDiff.events(prev, next, state, tick, LOW_RATIO);
    }

    private static String type(JsonObject frame) {
        return frame.get("event_type").getAsString();
    }

    /** 没有上一帧（首次连上 / 重连）就先出一份现状，别的都别提。 */
    @Test
    void firstFrameIsASnapshot() {
        List<JsonObject> frames = diff(null, snapshot("minecraft:desert"), 0L);

        assertEquals(1, frames.size());
        assertEquals("state_snapshot", type(frames.get(0)));
        assertEquals("minecraft:desert", frames.get(0).get("biome").getAsString());
        assertEquals("day", frames.get(0).get("phase").getAsString());
        assertEquals("standing", frames.get(0).get("posture").getAsString(), "现状里要说清她是什么姿势");
    }

    /** 换了监控的女仆：哪怕数据看着一样，也得重新交代一次。 */
    @Test
    void switchingMonitoredMaidResyncs() {
        PerceptionSnapshot other = new PerceptionSnapshot(
                "maid-2", "minecraft:overworld", "minecraft:plains", true, false, false, 1000L,
                "史蒂夫", 20.0, 20.0, "小狐", 20.0, 20.0, "standing");

        List<JsonObject> frames = diff(snapshot("minecraft:plains"), other, 100L);

        assertEquals(1, frames.size());
        assertEquals("state_snapshot", type(frames.get(0)));
    }

    /** 换维度时群系必然也变，只报维度那一条。 */
    @Test
    void dimensionChangeSwallowsBiomeChange() {
        List<JsonObject> frames = diff(
                snapshot("minecraft:overworld", "minecraft:plains", 20.0, 20.0),
                snapshot("minecraft:the_nether", "minecraft:nether_wastes", 20.0, 20.0),
                100L);
        assertEquals(1, frames.size());
        assertEquals("dimension_change", type(frames.get(0)));
        assertEquals("minecraft:overworld", frames.get(0).get("from_dimension").getAsString());
    }

    /** 鞘翅横穿一路的群系：冷静期内只更新记录，不推。 */
    @Test
    void biomeChangeIsRateLimited() {
        PerceptionSnapshot plains = snapshot("minecraft:plains");
        PerceptionSnapshot desert = snapshot("minecraft:desert");
        PerceptionSnapshot jungle = snapshot("minecraft:jungle");

        assertEquals(1, diff(plains, desert, 0L).size(), "第一次换群系该报");

        List<JsonObject> cooling = diff(desert, jungle, 100L);
        assertTrue(cooling.isEmpty(), "冷静期内不该再报");

        List<JsonObject> later = diff(jungle, plains, PerceptionDiff.BIOME_COOLDOWN_TICKS);
        assertEquals(1, later.size());
        assertEquals("biome_change", type(later.get(0)));
    }

    /** 昼夜按世界时间判：13000 那一刻入夜。 */
    @Test
    void dayTurnsIntoNightAtTheBoundary() {
        PerceptionSnapshot noon = withDayTime(12000L);
        PerceptionSnapshot dusk = withDayTime(13000L);

        List<JsonObject> frames = diff(noon, dusk, 0L);
        assertEquals(1, frames.size());
        assertEquals("time_phase_change", type(frames.get(0)));
        assertEquals("night", frames.get(0).get("phase").getAsString());
    }

    /** 没有天空的维度（下界 / 末地）不报天气与昼夜。 */
    @Test
    void weatherAndTimeAreSkippedWithoutSky() {
        PerceptionSnapshot before = new PerceptionSnapshot(
                "maid-1", "minecraft:the_nether", "minecraft:nether_wastes", false, false, false,
                12000L, "史蒂夫", 20.0, 20.0, "小狐", 20.0, 20.0, "standing");
        PerceptionSnapshot after = new PerceptionSnapshot(
                "maid-1", "minecraft:the_nether", "minecraft:nether_wastes", false, true, true,
                13000L, "史蒂夫", 20.0, 20.0, "小狐", 20.0, 20.0, "standing");

        assertTrue(diff(before, after, 0L).isEmpty());
    }

    /** 血量告警是边沿触发的：跌破报一次，回血复位后才允许再报。 */
    @Test
    void lowHealthFiresOnceAndRearms() {
        PerceptionSnapshot healthy = snapshot("minecraft:plains");
        PerceptionSnapshot hurt = snapshot("minecraft:overworld", "minecraft:plains", 4.0, 20.0);

        List<JsonObject> first = diff(healthy, hurt, 0L);
        assertEquals(1, first.size());
        assertEquals("low_health", type(first.get(0)));
        assertEquals("player", first.get(0).get("who").getAsString());

        PerceptionSnapshot worse = snapshot("minecraft:overworld", "minecraft:plains", 3.0, 20.0);
        assertTrue(diff(hurt, worse, 100L).isEmpty(), "还在低血状态里不该反复报");

        // 回到阈值 ×1.2 以上才算缓过来
        PerceptionSnapshot recoveredHealth = snapshot("minecraft:overworld", "minecraft:plains", 19.0, 20.0);
        assertTrue(diff(worse, recoveredHealth, 200L).isEmpty());
        assertEquals(1, diff(recoveredHealth, hurt, 300L).size(), "复位后再跌破要能再报");
    }

    /** 女仆血量同样算，且和玩家分开记。 */
    @Test
    void maidLowHealthIsTrackedSeparately() {
        List<JsonObject> frames = diff(
                snapshot("minecraft:plains"),
                snapshot("minecraft:overworld", "minecraft:plains", 20.0, 3.0),
                0L);

        assertEquals(1, frames.size());
        assertEquals("maid", frames.get(0).get("who").getAsString());
    }

    /** 昏迷时世界变化一律不推；重连仍要交代现状，否则模型不知道她倒下了。 */
    @Test
    void unconsciousSkipsWorldChanges() {
        PerceptionSnapshot down = new PerceptionSnapshot(
                "maid-1", "minecraft:overworld", "minecraft:desert", true, true, true, 13000L,
                "史蒂夫", 4.0, 20.0, "小狐", 1.0, 20.0, "unconscious");

        assertTrue(diff(snapshot("minecraft:plains"), down, 0L).isEmpty());

        List<JsonObject> first = diff(null, down, 0L);
        assertEquals(1, first.size());
        assertEquals("state_snapshot", type(first.get(0)));
        assertEquals("unconscious", first.get(0).get("posture").getAsString());
    }

    private static PerceptionSnapshot withDayTime(long dayTime) {
        return new PerceptionSnapshot(
                "maid-1", "minecraft:overworld", "minecraft:plains", true, false, false, dayTime,
                "史蒂夫", 20.0, 20.0, "小狐", 20.0, 20.0, "standing");
    }
}
