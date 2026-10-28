package com.lingchat.maidbridge.perception.nearby;

import static org.junit.jupiter.api.Assertions.assertEquals;
import org.junit.jupiter.api.Test;

/** 头顶三档的判定口径：只看这一圈里还有没有露天的列。 */
class OverheadScanTest {
    @Test
    void anyOpenColumnNearbyMeansPartial() {
        assertEquals(OverheadScan.PARTIAL, OverheadScan.classify(1), "树荫、屋檐、洞口");
        assertEquals(OverheadScan.PARTIAL, OverheadScan.classify(12));
    }

    @Test
    void noOpenColumnMeansEnclosed() {
        assertEquals(OverheadScan.ENCLOSED, OverheadScan.classify(0), "屋里、洞里");
    }

    @Test
    void coverBandSplitsAtThreeBlocks() {
        assertEquals(OverheadScan.BAND_LOW, OverheadScan.coverBand(0), "紧贴头顶");
        assertEquals(OverheadScan.BAND_LOW, OverheadScan.coverBand(OverheadScan.LOW_BAND_GAP));
        assertEquals(OverheadScan.BAND_HIGH, OverheadScan.coverBand(OverheadScan.LOW_BAND_GAP + 1));
    }
}
