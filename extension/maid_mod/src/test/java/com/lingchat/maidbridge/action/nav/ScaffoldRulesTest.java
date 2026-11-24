package com.lingchat.maidbridge.action.nav;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class ScaffoldRulesTest {
    @Test
    void stepsWhenTargetIsLevelOrOneAbove() {
        // 屋顶那一步：她与屋顶齐平、或者只差一格，都还得垫
        assertTrue(ScaffoldRules.mayStep(0));
        assertTrue(ScaffoldRules.mayStep(1));
        assertTrue(ScaffoldRules.mayStep(2));
        assertTrue(ScaffoldRules.mayStep(ScaffoldRules.MAX_RISE));
    }

    @Test
    void doesNotStepBelowFeetOrBeyondReach() {
        assertFalse(ScaffoldRules.mayStep(-1));
        assertFalse(ScaffoldRules.mayStep(ScaffoldRules.MAX_RISE + 1));
    }

    @Test
    void onlyTallClimbsMustKeepApproaching() {
        // 往上爬的途中必须逐步接近；齐平或只高一格时，为了站到放得下的位置可以绕
        assertFalse(ScaffoldRules.mustApproach(0));
        assertFalse(ScaffoldRules.mustApproach(1));
        assertTrue(ScaffoldRules.mustApproach(2));
        assertTrue(ScaffoldRules.mustApproach(8));
    }

    @Test
    void bridgesOnlyAtTheSameHeight() {
        // 齐平的缺口才平着铺；高一格交给垫脚，低一格交给跑跳
        assertTrue(ScaffoldRules.mayBridge(0));
        assertFalse(ScaffoldRules.mayBridge(1));
        assertFalse(ScaffoldRules.mayBridge(-1));
    }

    @Test
    void bridgeWalksTheWiderAxisFirst() {
        assertTrue(ScaffoldRules.alongX(5, 2));
        assertFalse(ScaffoldRules.alongX(2, -5));
        // 两个方向一样远时先横着，跟候选站位的尝试顺序一致
        assertTrue(ScaffoldRules.alongX(-3, 3));
    }

    @Test
    void horizontalReachIsCheckedPerAxis() {
        assertTrue(ScaffoldRules.nearEnough(0, 0));
        assertTrue(
                ScaffoldRules.nearEnough(-ScaffoldRules.MAX_DISTANCE, ScaffoldRules.MAX_DISTANCE));
        assertFalse(ScaffoldRules.nearEnough(ScaffoldRules.MAX_DISTANCE + 1, 0));
        assertFalse(ScaffoldRules.nearEnough(0, -ScaffoldRules.MAX_DISTANCE - 1));
    }
}
