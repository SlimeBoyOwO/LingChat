package com.lingchat.maidbridge.action.mine;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
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

    @Test
    void offsetAcceptsTheFormThePluginProduces() {
        MineRules.Offset offset = MineRules.parseOffset("@4,-2,-3");
        assertEquals(4, offset.dx());
        assertEquals(-2, offset.dy());
        assertEquals(-3, offset.dz());
        // 两头的 @ 可有可无，空白也认：模型抄地址时未必一字不差
        assertEquals(offset, MineRules.parseOffset(" 4 , -2 , -3 "));
        // 整句抄回来时前面的 at= 也认
        assertEquals(offset, MineRules.parseOffset("at=@4,-2,-3"));
        assertEquals("@4,-2,-3", MineRules.formatOffset(4, -2, -3));
    }

    @Test
    void malformedOffsetsAreRejected() {
        assertNull(MineRules.parseOffset(""));
        assertNull(MineRules.parseOffset("@"));
        assertNull(MineRules.parseOffset("@4,-2"));
        assertNull(MineRules.parseOffset("@4,-2,-3,-1"));
        assertNull(MineRules.parseOffset("@4,two,-3"));
        assertNull(MineRules.parseOffset("@4,,-3"));
        // 超出一眼能看见的范围：多半是模型自己编的坐标
        assertNull(MineRules.parseOffset("@999,0,0"));
    }
}
