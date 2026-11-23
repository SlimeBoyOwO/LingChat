package com.lingchat.maidbridge.action.construction;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.lingchat.maidbridge.action.construction.BuildRules.Action;
import com.lingchat.maidbridge.action.construction.BuildRules.CellState;
import com.lingchat.maidbridge.action.construction.BuildRules.PassEnd;
import com.lingchat.maidbridge.action.construction.BuildRules.PassStats;
import org.junit.jupiter.api.Test;

/** 施工的纯规则：不碰 Minecraft，只验状态和分层收尾的去向。 */
class BuildRulesTest {
    /** 一层扫完的常规输入：还没全部放完，本层还有开格，有进展，可以换层。 */
    private static PassStats stats(
            boolean allPlaced, boolean layerOpen, boolean openHeld, boolean progress,
            int stall, boolean retried, boolean canMoveOn, boolean canRetry) {
        return new PassStats(allPlaced, layerOpen, openHeld, progress, stall, retried,
                canMoveOn, canRetry);
    }

    private static PassStats normal() {
        return stats(false, true, false, true, 0, false, true, false);
    }

    @Test
    void openStatesArePendingAndRetrying() {
        assertTrue(CellState.PENDING.open());
        assertTrue(CellState.RETRYING.open());
        assertFalse(CellState.DEFERRED.open());
        assertFalse(CellState.DONE.open());
        assertFalse(CellState.FAILED.open());
    }

    @Test
    void failureCodesKeepWireFormat() {
        assertEquals("blocked", BuildRules.Failure.OCCUPIED.code);
        assertEquals("blocked_by_entity", BuildRules.Failure.BLOCKED_BY_ENTITY.code);
        assertEquals("short_of_material", BuildRules.Failure.SHORT_OF_MATERIAL.code);
    }

    @Test
    void allPlacedFinishes() {
        PassEnd end = BuildRules.endPass(stats(true, true, false, true, 0, false, true, false));
        assertEquals(Action.FINISH, end.first());
        assertEquals(Action.NONE, end.second());
    }

    @Test
    void closedLayerMovesOnWhenAnotherLayerIsOpen() {
        PassEnd end = BuildRules.endPass(stats(false, false, false, false, 0, false, true, true));
        assertEquals(Action.NEXT_LAYER, end.first());
    }

    @Test
    void closedLayerRetriesBeforeFinishing() {
        PassEnd end = BuildRules.endPass(stats(false, false, false, false, 0, false, false, true));
        assertEquals(Action.RETRY_ROUND, end.first());
    }

    @Test
    void closedLayerFinishesWithoutWorkLeft() {
        PassEnd end = BuildRules.endPass(stats(false, false, false, false, 0, false, false, false));
        assertEquals(Action.FINISH, end.first());
    }

    @Test
    void progressOrHeldCellsKeepScanning() {
        assertEquals(Action.RESCAN, BuildRules.endPass(normal()).first());
        assertEquals(Action.RESCAN,
                BuildRules.endPass(stats(false, true, true, false, 2, false, true, false)).first());
    }

    @Test
    void stallKeepsScanningUntilLimit() {
        assertEquals(Action.RESCAN,
                BuildRules.endPass(stats(false, true, false, false, 1, false, true, false)).first());
        assertEquals(Action.RESCAN,
                BuildRules.endPass(stats(false, true, false, false, 2, false, true, false)).first());
    }

    @Test
    void stalledLayerDefersBeforeRetrying() {
        PassEnd end = BuildRules.endPass(stats(false, true, false, false, 3, false, true, true));
        assertEquals(Action.DEFER_LAYER, end.first());
        assertEquals(Action.NEXT_LAYER, end.second());
    }

    @Test
    void stalledLayerSettlesAfterRetryRound() {
        PassEnd end = BuildRules.endPass(stats(false, true, false, false, 3, true, true, false));
        assertEquals(Action.SETTLE_LAYER, end.first());
        assertEquals(Action.NEXT_LAYER, end.second());
    }

    @Test
    void stalledLayerCanRetryWhenNothingElseToDo() {
        // 搁置之后本层就有可补试的格子了，不必事先已经存在 deferred
        PassEnd end = BuildRules.endPass(stats(false, true, false, false, 3, false, false, false));
        assertEquals(Action.DEFER_LAYER, end.first());
        assertEquals(Action.RETRY_ROUND, end.second());
    }

    @Test
    void stalledLayerFinishesWhenNoWorkLeft() {
        PassEnd end = BuildRules.endPass(stats(false, true, false, false, 3, true, false, false));
        assertEquals(Action.SETTLE_LAYER, end.first());
        assertEquals(Action.FINISH, end.second());
    }

    private static BuildRules.Situation situation(
            boolean matches, boolean liquid, boolean occupied, boolean canDestroy,
            boolean hasMaterial, boolean entityHere, boolean selfHere) {
        return new BuildRules.Situation(matches, liquid, occupied, canDestroy,
                hasMaterial, entityHere, selfHere);
    }

    private static BuildRules.Situation clear() {
        return situation(false, false, false, false, true, false, false);
    }

    @Test
    void finishedCellWinsOverEverything() {
        assertEquals(BuildRules.Verdict.DONE,
                BuildRules.verdict(situation(true, true, true, false, false, true, true)));
    }

    @Test
    void liquidIsNeverBuilt() {
        assertEquals(BuildRules.Verdict.LIQUID,
                BuildRules.verdict(situation(false, true, false, true, true, true, true)));
    }

    @Test
    void occupiedCellIsDugOrLeftAlone() {
        assertEquals(BuildRules.Verdict.DIG,
                BuildRules.verdict(situation(false, false, true, true, true, false, false)));
        assertEquals(BuildRules.Verdict.DIG_BLOCKED,
                BuildRules.verdict(situation(false, false, true, false, true, false, false)));
    }

    @Test
    void materialComesBeforeEntities() {
        assertEquals(BuildRules.Verdict.MISSING,
                BuildRules.verdict(situation(false, false, false, false, false, true, true)));
        assertEquals(BuildRules.Verdict.HELD_BY_ENTITY,
                BuildRules.verdict(situation(false, false, false, false, true, true, true)));
    }

    @Test
    void sheMovesOutBeforePlacing() {
        assertEquals(BuildRules.Verdict.SELF_INSIDE,
                BuildRules.verdict(situation(false, false, false, false, true, false, true)));
    }

    @Test
    void clearCellNeedsReachAndSight() {
        assertEquals(BuildRules.Verdict.CONTINUE, BuildRules.verdict(clear()));
    }
}
