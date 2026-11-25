package com.lingchat.maidbridge.action.nav;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import net.minecraft.core.BlockPos;
import org.junit.jupiter.api.Test;

/** 站位占格的估算：按格心站好时，身体会不会探进目标格。 */
class BodiesTest {
    private static final double WIDTH = 0.6;
    private static final double HEIGHT = 1.8;
    private static final BlockPos STAND = new BlockPos(0, 64, 0);

    @Test
    void adjacentStandDoesNotTouchTheTarget() {
        assertFalse(Bodies.bodyOverlaps(STAND, WIDTH, HEIGHT, new BlockPos(1, 64, 0)));
        assertFalse(Bodies.bodyOverlaps(STAND, WIDTH, HEIGHT, new BlockPos(0, 64, 1)));
    }

    @Test
    void diagonalStandDoesNotTouchTheTarget() {
        assertFalse(Bodies.bodyOverlaps(STAND, WIDTH, HEIGHT, new BlockPos(1, 64, 1)));
    }

    @Test
    void targetAboveSharesTheHeadCell() {
        assertTrue(Bodies.bodyOverlaps(STAND, WIDTH, HEIGHT, new BlockPos(0, 65, 0)));
    }

    @Test
    void widerBodyReachesTheNeighbour() {
        assertTrue(Bodies.bodyOverlaps(STAND, 1.2, HEIGHT, new BlockPos(1, 64, 0)));
    }

    @Test
    void floorBelowIsNotOccupied() {
        assertFalse(Bodies.bodyOverlaps(STAND, WIDTH, HEIGHT, new BlockPos(0, 63, 0)));
    }
}
