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
            boolean allPlaced, boolean layerHasOpen, boolean openHeld, boolean progress,
            int stall, boolean retried, boolean anotherLayerHasWork, boolean deferredLeft) {
        return new PassStats(allPlaced, layerHasOpen, openHeld, progress, stall, retried,
                anotherLayerHasWork, deferredLeft);
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
    void deferredCanEnterTheRetryRound() {
        assertTrue(CellFlow.allowed(CellState.DEFERRED, CellState.RETRYING));
        assertTrue(CellFlow.allowed(CellState.DEFERRED, CellState.FAILED));
        assertFalse(CellFlow.allowed(CellState.FAILED, CellState.RETRYING));
        assertFalse(CellFlow.allowed(CellState.DONE, CellState.RETRYING));
        assertFalse(CellFlow.allowed(CellState.PENDING, CellState.PENDING));
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
        assertEquals(Action.FINISH, end.actions().get(0));
        assertEquals(1, end.actions().size());
    }

    @Test
    void closedLayerMovesOnWhenAnotherLayerIsOpen() {
        PassEnd end = BuildRules.endPass(stats(false, false, false, false, 0, false, true, true));
        assertEquals(Action.NEXT_LAYER, end.actions().get(0));
    }

    @Test
    void closedLayerRetriesBeforeFinishing() {
        PassEnd end = BuildRules.endPass(stats(false, false, false, false, 0, false, false, true));
        assertEquals(Action.RETRY_ROUND, end.actions().get(0));
        // 只有这一个动作：以前写成两个槽位时，执行侧漏接过第一槽
        assertEquals(1, end.actions().size());
    }

    /**
     * 全组合扫一遍：每个去向至少有一个动作。
     *
     * <p>执行器是按枚举穷尽 switch 的，这条测试盯的是决策表本身别产出空管线。
     */
    @Test
    void everyCombinationYieldsRealActions() {
        for (int mask = 0; mask < 128; mask++) {
            for (int stall = 0; stall < 4; stall++) {
                PassEnd end = BuildRules.endPass(stats(
                        bit(mask, 0), bit(mask, 1), bit(mask, 2), bit(mask, 3),
                        stall, bit(mask, 4), bit(mask, 5), bit(mask, 6)));
                assertFalse(end.actions().isEmpty());
            }
        }
    }

    private static boolean bit(int mask, int index) {
        return (mask & (1 << index)) != 0;
    }

    @Test
    void closedLayerFinishesWithoutWorkLeft() {
        PassEnd end = BuildRules.endPass(stats(false, false, false, false, 0, false, false, false));
        assertEquals(Action.FINISH, end.actions().get(0));
    }

    @Test
    void progressOrHeldCellsKeepScanning() {
        assertEquals(Action.RESCAN, BuildRules.endPass(normal()).actions().get(0));
        assertEquals(Action.RESCAN,
                BuildRules.endPass(stats(false, true, true, false, 2, false, true, false)).actions().get(0));
    }

    @Test
    void stallKeepsScanningUntilLimit() {
        assertEquals(Action.RESCAN,
                BuildRules.endPass(stats(false, true, false, false, 1, false, true, false)).actions().get(0));
        assertEquals(Action.RESCAN,
                BuildRules.endPass(stats(false, true, false, false, 2, false, true, false)).actions().get(0));
    }

    @Test
    void stalledLayerDefersBeforeRetrying() {
        PassEnd end = BuildRules.endPass(stats(false, true, false, false, 3, false, true, true));
        assertEquals(Action.DEFER_LAYER, end.actions().get(0));
        assertEquals(Action.NEXT_LAYER, end.actions().get(1));
    }

    @Test
    void stalledLayerSettlesAfterRetryRound() {
        PassEnd end = BuildRules.endPass(stats(false, true, false, false, 3, true, true, false));
        assertEquals(Action.SETTLE_LAYER, end.actions().get(0));
        assertEquals(Action.NEXT_LAYER, end.actions().get(1));
    }

    @Test
    void stalledLayerCanRetryWhenNothingElseToDo() {
        // 搁置之后本层就有可补试的格子了，不必事先已经存在 deferred
        PassEnd end = BuildRules.endPass(stats(false, true, false, false, 3, false, false, false));
        assertEquals(Action.DEFER_LAYER, end.actions().get(0));
        assertEquals(Action.RETRY_ROUND, end.actions().get(1));
    }

    @Test
    void stalledLayerFinishesWhenNoWorkLeft() {
        PassEnd end = BuildRules.endPass(stats(false, true, false, false, 3, true, false, false));
        assertEquals(Action.SETTLE_LAYER, end.actions().get(0));
        assertEquals(Action.FINISH, end.actions().get(1));
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
