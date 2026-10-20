package com.lingchat.maidbridge.action.mine;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

/** 挖掘算式与上限：纯逻辑，不碰 Minecraft。 */
class MineRulesTest {
    private static final float EPSILON = 1.0E-6F;

    @Test
    void correctToolMatchesVanillaSpeed() {
        // 铁镐挖石头（硬度 1.5）：6 / 1.5 / 30 = 0.133… 每 tick，约 8 tick 一块
        assertEquals(6.0F / 1.5F / 30.0F, MineRules.progressPerTick(1.5F, 6.0F, true), EPSILON);
    }

    @Test
    void wrongToolIsThreeTimesSlowerThanCorrect() {
        float correct = MineRules.progressPerTick(3.0F, 4.0F, true);
        float wrong = MineRules.progressPerTick(3.0F, 4.0F, false);
        assertEquals(correct / 3.3333333F, wrong, 1.0E-5F);
    }

    @Test
    void unbreakableBlocksNeverProgress() {
        assertEquals(0.0F, MineRules.progressPerTick(-1.0F, 8.0F, true), EPSILON);
        assertFalse(MineRules.mineable(-1.0F));
    }

    @Test
    void zeroHardnessBreaksImmediately() {
        assertEquals(1.0F, MineRules.progressPerTick(0.0F, 1.0F, false), EPSILON);
    }

    @Test
    void toolSpeedNeverDropsBelowHand() {
        // 空手或手滑的时候原版也是按 1 算，不是 0
        assertEquals(1.0F / 2.0F / 100.0F, MineRules.progressPerTick(2.0F, 0.0F, false), EPSILON);
    }

    @Test
    void crackStageMatchesVanillaDisplay() {
        assertEquals(-1, MineRules.crackStage(0.0F));
        assertEquals(0, MineRules.crackStage(0.05F));
        assertEquals(3, MineRules.crackStage(0.45F));
        assertEquals(8, MineRules.crackStage(0.95F));
        assertEquals(9, MineRules.crackStage(1.0F));
    }

    @Test
    void countIsClamped() {
        assertEquals(1, MineRules.clampCount(0));
        assertEquals(1, MineRules.clampCount(-5));
        assertEquals(7, MineRules.clampCount(7));
        assertEquals(MineRules.MAX_COUNT, MineRules.clampCount(999));
    }

    @Test
    void reachLeavesRoomForStandingNextToABlock() {
        assertTrue(MineRules.REACH > 3.0);
        assertTrue(MineRules.REACH < 6.0);
    }
}
