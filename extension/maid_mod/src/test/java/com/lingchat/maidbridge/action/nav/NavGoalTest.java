package com.lingchat.maidbridge.action.nav;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import net.minecraft.core.BlockPos;
import org.junit.jupiter.api.Test;

/** 走到附近的目标：要求同层时，坎下离得再近也不算到。 */
class NavGoalTest {
    private static final BlockPos TARGET = new BlockPos(0, 64, 0);

    @Test
    void nearCountsDistanceOnlyWhenLevelIsNotRequired() {
        NavGoal.Near plain = new NavGoal.Near(TARGET, 3, false);
        assertTrue(plain.accepts(new BlockPos(2, 63, 0)));
    }

    @Test
    void nearRequiresTheTargetLevelWhenAsked() {
        NavGoal.Near level = new NavGoal.Near(TARGET, 3, true);
        assertFalse(level.accepts(new BlockPos(2, 63, 0)));
        assertTrue(level.accepts(new BlockPos(2, 64, 0)));
        assertFalse(level.accepts(new BlockPos(5, 64, 0)));
    }
}
