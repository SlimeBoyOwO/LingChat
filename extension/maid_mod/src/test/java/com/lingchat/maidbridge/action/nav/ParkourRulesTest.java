package com.lingchat.maidbridge.action.nav;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.Set;
import net.minecraft.core.BlockPos;
import org.junit.jupiter.api.Test;

/** 跑跳的距离与高差规则：纯逻辑，不碰 Minecraft。 */
class ParkourRulesTest {
    @Test
    void flatJumpReachesFourBlocks() {
        assertTrue(ParkourRules.allowed(2, 0));
        assertTrue(ParkourRules.allowed(3, 0));
        assertTrue(ParkourRules.allowed(4, 0));
        assertFalse(ParkourRules.allowed(5, 0));
    }

    @Test
    void adjacentStepIsNotAParkourHop() {
        assertFalse(ParkourRules.allowed(1, 0));
        assertFalse(ParkourRules.allowed(1, 1));
    }

    @Test
    void risingCutsReachToThree() {
        assertTrue(ParkourRules.allowed(3, 1));
        assertFalse(ParkourRules.allowed(4, 1));
    }

    @Test
    void cannotJumpTwoUpOrFourDown() {
        assertFalse(ParkourRules.allowed(2, 2));
        assertFalse(ParkourRules.allowed(4, -4));
        assertTrue(ParkourRules.allowed(4, -3));
    }

    @Test
    void deepDropOnlyOpensWhenAsked() {
        assertFalse(ParkourRules.allowed(2, -4));
        assertTrue(ParkourRules.allowed(2, -4, ParkourRules.DEEP_FALL));
        assertTrue(ParkourRules.allowed(1, -4, ParkourRules.DEEP_FALL));
        assertFalse(ParkourRules.allowed(1, 0, ParkourRules.DEEP_FALL));
        assertFalse(ParkourRules.allowed(2, -21, ParkourRules.DEEP_FALL));
    }

    @Test
    void fallingStaysAirborneLonger() {
        assertEquals(ParkourRules.FLIGHT_TICKS, ParkourRules.flightTicks(0));
        assertEquals(ParkourRules.FLIGHT_TICKS, ParkourRules.flightTicks(1));
        assertEquals(ParkourRules.FLIGHT_TICKS + 2, ParkourRules.flightTicks(-1));
        assertEquals(ParkourRules.FLIGHT_TICKS + 6, ParkourRules.flightTicks(-3));
    }

    @Test
    void diagonalJumpsUseStraightLineDistance() {
        int fall = ParkourRules.MAX_FALL;
        assertTrue(ParkourRules.allowed(1, 1, 0, fall));
        assertTrue(ParkourRules.allowed(3, 2, 0, fall));
        assertFalse(ParkourRules.allowed(3, 3, 0, fall));
        assertTrue(ParkourRules.allowed(2, 2, 1, fall));
        assertFalse(ParkourRules.allowed(3, 1, 1, fall));
        assertFalse(ParkourRules.allowed(1, 0, 0, fall));
    }

    @Test
    void sweptColumnsFollowTheFlightLine() {
        BlockPos from = new BlockPos(0, 5, 0);
        assertEquals(Set.of(new BlockPos(1, 5, 0), new BlockPos(2, 5, 0), new BlockPos(3, 5, 0)),
                MaidParkour.sweptColumns(from, new BlockPos(4, 5, 0)));
        // 斜对角一格：擦过的两个墙角都要查
        assertEquals(Set.of(new BlockPos(1, 5, 0), new BlockPos(0, 5, 1)),
                MaidParkour.sweptColumns(from, new BlockPos(1, 6, 1)));
    }
}
