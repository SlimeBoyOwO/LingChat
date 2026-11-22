package com.lingchat.maidbridge.action.construction;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

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
    void fallingStaysAirborneLonger() {
        assertEquals(ParkourRules.FLIGHT_TICKS, ParkourRules.flightTicks(0));
        assertEquals(ParkourRules.FLIGHT_TICKS, ParkourRules.flightTicks(1));
        assertEquals(ParkourRules.FLIGHT_TICKS + 2, ParkourRules.flightTicks(-1));
        assertEquals(ParkourRules.FLIGHT_TICKS + 6, ParkourRules.flightTicks(-3));
    }
}
