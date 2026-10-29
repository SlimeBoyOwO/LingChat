package com.lingchat.maidbridge.action.place;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import org.junit.jupiter.api.Test;

/** 放置方向的口径：协议 token 与原版 yaw 必须对得上。 */
class PlaceRulesTest {
    @Test
    void yawMatchesVanillaDirections() {
        assertEquals(0.0F, PlaceRules.yawOf("south"), 0.001F);
        assertEquals(90.0F, PlaceRules.yawOf("west"), 0.001F);
        assertEquals(180.0F, PlaceRules.yawOf("north"), 0.001F);
        assertEquals(270.0F, PlaceRules.yawOf("east"), 0.001F);
        // 模型可能带空白或大小写
        assertEquals(180.0F, PlaceRules.yawOf(" NORTH "), 0.001F);
    }

    @Test
    void unknownFacingFallsBackToHerOwn() {
        assertNull(PlaceRules.yawOf(""));
        assertNull(PlaceRules.yawOf("up"));
        assertNull(PlaceRules.yawOf(null));
    }
}
