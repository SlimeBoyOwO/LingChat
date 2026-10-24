package com.lingchat.maidbridge.action.core;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import org.junit.jupiter.api.Test;

/** 挑床：纯逻辑，能脱开游戏跑。 */
class BedChoiceTest {

    private static BedChoice.Candidate bed(int x, int z) {
        return new BedChoice.Candidate(x, 64, z, true, true, false);
    }

    @Test
    void bedPoiIdsCoverVanillaAndMaidBeds() {
        assertTrue(BedChoice.isBedPoiId("minecraft:home"));
        assertTrue(BedChoice.isBedPoiId("touhou_little_maid:maid_bed"));
        assertFalse(BedChoice.isBedPoiId("minecraft:armorer"));
        assertFalse(BedChoice.isBedPoiId(""));
    }

    @Test
    void nearestBedWins() {
        List<BedChoice.Candidate> candidates = Arrays.asList(bed(30, 0), bed(5, 0), bed(12, 0));

        BedChoice.Pick pick = BedChoice.pick(candidates, 0, 64, 0);

        assertEquals(5, pick.pos().x());
        assertFalse(pick.sawOccupied());
    }

    /** 等距时必须给出固定的一侧：否则每次挑出来的床都不同，行为不可复现。 */
    @Test
    void equalDistanceIsStable() {
        List<BedChoice.Candidate> forward = Arrays.asList(bed(5, 0), bed(-5, 0));
        List<BedChoice.Candidate> backward = Arrays.asList(bed(-5, 0), bed(5, 0));

        assertEquals(-5, BedChoice.pick(forward, 0, 64, 0).pos().x());
        assertEquals(-5, BedChoice.pick(backward, 0, 64, 0).pos().x());
    }

    @Test
    void occupiedBedIsSkippedAndReported() {
        List<BedChoice.Candidate> candidates = Arrays.asList(bed(3, 0), bed(9, 0));
        // 最近那张被占了
        candidates.set(0, new BedChoice.Candidate(3, 64, 0, true, true, true));

        BedChoice.Pick pick = BedChoice.pick(candidates, 0, 64, 0);

        assertEquals(9, pick.pos().x());
        assertTrue(pick.sawOccupied());
    }

    @Test
    void allOccupiedMeansNoBedButOccupancyIsReported() {
        List<BedChoice.Candidate> candidates = Collections.singletonList(
                new BedChoice.Candidate(3, 64, 0, true, true, true));

        BedChoice.Pick pick = BedChoice.pick(candidates, 0, 64, 0);

        assertNull(pick.pos());
        assertTrue(pick.sawOccupied(), "要能说清是「床都被占了」而不是「没有床」");
    }

    /** POI 索引会过期：床被挖了、被换成别的方块时，以方块事实为准。 */
    @Test
    void staleIndexEntriesAreIgnored() {
        List<BedChoice.Candidate> candidates = Arrays.asList(
                new BedChoice.Candidate(1, 64, 0, false, false, false),
                new BedChoice.Candidate(2, 64, 0, true, false, false));

        BedChoice.Pick pick = BedChoice.pick(candidates, 0, 64, 0);

        assertNull(pick.pos());
        assertFalse(pick.sawOccupied());
    }

    @Test
    void noCandidatesIsNoBed() {
        BedChoice.Pick pick = BedChoice.pick(Collections.emptyList(), 0, 64, 0);

        assertNull(pick.pos());
        assertFalse(pick.sawOccupied());
    }
}
