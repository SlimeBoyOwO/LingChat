package com.lingchat.maidbridge.action.core;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import org.junit.jupiter.api.Test;

/**
 * 动作状态机：这部分是纯逻辑，能脱开游戏跑。
 *
 * <p>要守住的是「什么时候算成、什么时候如实认输」：到位阈值两侧、卡在床边、走不过去、
 * 躺下之后被自己叫起来、看向玩家到时收尾，以及「她不见了」优先于其他一切判定。
 */
class ActionRunTest {

    private static MaidAction action(MaidAction.Kind kind, MaidAction.Target target) {
        return new MaidAction(kind, target);
    }

    private static ActionRun.Observation still(int distance) {
        return new ActionRun.Observation(true, false, false, true, false, distance, distance);
    }

    private static ActionRun.Observation observation(
            boolean maidPresent, boolean blocked, boolean sleeping, boolean targetPresent, int distance) {
        return new ActionRun.Observation(maidPresent, blocked, sleeping, targetPresent, false, distance, distance);
    }

    private static ActionRun.Outcome tick(ActionRun run, ActionRun.Observation observation) {
        return run.tick(observation);
    }

    @Test
    void walksUntilCloseEnough() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.GO_TO, MaidAction.Target.PLAYER));

        ActionRun.Outcome far = tick(run, still(4));
        assertFalse(far.finished());
        assertEquals(ActionRun.Command.WALK, far.command());

        ActionRun.Outcome arrived = tick(run, still(3));
        assertTrue(arrived.finished());
        assertEquals("done", arrived.status());
    }

    @Test
    void bedTargetUsesTheTighterThreshold() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.GO_TO, MaidAction.Target.BED));

        assertFalse(tick(run, still(3)).finished(), "床边要走到曼哈顿 2 格才算到位");
        assertTrue(tick(run, still(2)).finished());
    }

    @Test
    void givesUpAtTimeout() {
        MaidAction goTo = action(MaidAction.Kind.GO_TO, MaidAction.Target.PLAYER);
        ActionRun run = new ActionRun(goTo);

        ActionRun.Outcome outcome = null;
        for (int i = 0; i < goTo.timeoutTicks(); i++) {
            outcome = tick(run, still(20));
        }

        assertTrue(outcome.finished());
        assertEquals("failed", outcome.status());
        assertEquals("timeout", outcome.reasonCode());
    }

    /** 只 tick 十次不该超时：暂停时 ServerTickEvent.Post 不触发，暂停天然不计时。 */
    @Test
    void pausedTicksDoNotCountTowardsTheTimeout() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.GO_TO, MaidAction.Target.PLAYER));

        for (int i = 0; i < 10; i++) {
            assertFalse(tick(run, still(20)).finished());
        }
    }

    /** 路被挡住时要早于超时认输，别让玩家干等三十秒。 */
    @Test
    void unreachableFailsBeforeTheTimeout() {
        MaidAction goTo = action(MaidAction.Kind.GO_TO, MaidAction.Target.PLAYER);
        ActionRun run = new ActionRun(goTo);
        ActionRun.Observation blockedPath =
                new ActionRun.Observation(true, false, false, true, true, 10, 10.0);

        ActionRun.Outcome outcome = null;
        for (int i = 0; i < ActionRun.UNREACHABLE_TICKS; i++) {
            outcome = tick(run, blockedPath);
        }

        assertTrue(outcome.finished());
        assertEquals("unreachable", outcome.reasonCode());
        assertTrue(run.elapsedTicks() < goTo.timeoutTicks());
    }

    @Test
    void aTransientUnreachableFlagIsForgiven() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.GO_TO, MaidAction.Target.PLAYER));
        ActionRun.Observation blockedPath =
                new ActionRun.Observation(true, false, false, true, true, 10, 10.0);

        for (int i = 0; i < ActionRun.UNREACHABLE_TICKS - 1; i++) {
            tick(run, blockedPath);
        }

        assertFalse(tick(run, still(10)).finished(), "标记断了就该重新数，路径偶尔算不出来是常事");
    }

    /** 床边那一格常常走不进去：贴近了又不再靠近，就当到了。 */
    @Test
    void stuckNextToTheBedCountsAsArrived() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.LIE_DOWN, MaidAction.Target.BED));

        ActionRun.Outcome outcome = null;
        for (int i = 0; i < ActionRun.STALL_TICKS + 1; i++) {
            outcome = tick(run, still(ActionRun.NEAR_ENOUGH));
        }

        assertTrue(outcome.finished());
        assertEquals("done", outcome.status());
    }

    @Test
    void beingAbleToMoveIsRequired() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.GO_TO, MaidAction.Target.PLAYER));

        ActionRun.Outcome outcome = tick(run, observation(true, true, false, true, 10));

        assertEquals("failed", outcome.status());
        assertEquals("maid_busy", outcome.reasonCode());
    }

    @Test
    void targetDisappearingEndsTheRun() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.GO_TO, MaidAction.Target.PLAYER));

        ActionRun.Outcome outcome = tick(run, observation(true, false, false, false, 10));

        assertEquals("failed", outcome.status());
        assertEquals("target_gone", outcome.reasonCode());
    }

    /** 她不见了这条最优先：别的判定都依赖她还在。 */
    @Test
    void maidGoneWinsOverEverythingElse() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.GO_TO, MaidAction.Target.PLAYER));

        ActionRun.Outcome outcome = tick(run, new ActionRun.Observation(false, true, true, false, true, 99, 99));

        assertEquals("failed", outcome.status());
        assertEquals("maid_gone", outcome.reasonCode());
    }

    @Test
    void lyingDownWalksThenSleeps() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.LIE_DOWN, MaidAction.Target.BED));

        assertEquals(ActionRun.Command.WALK, tick(run, still(6)).command());

        ActionRun.Outcome reachedBed = tick(run, still(2));
        assertFalse(reachedBed.finished(), "贴到床边只是走到，还没躺下");
        assertEquals(ActionRun.Command.SLEEP, reachedBed.command());

        ActionRun.Observation asleep = observation(true, true, true, true, 1);
        ActionRun.Outcome outcome = null;
        for (int i = 0; i < ActionRun.SLEEP_CONFIRM_TICKS; i++) {
            outcome = tick(run, asleep);
        }

        assertTrue(outcome.finished());
        assertEquals("done", outcome.status());
    }

    /** 日程不在睡觉时段时，车万女仆下一 tick 就会把她叫起来——这时要如实说「现在不困」。 */
    @Test
    void standingBackUpIsReportedAsNotSleepTime() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.LIE_DOWN, MaidAction.Target.BED));
        tick(run, still(2));

        ActionRun.Outcome outcome = tick(run, observation(true, false, false, true, 2));

        assertEquals("failed", outcome.status());
        assertEquals("not_sleep_time", outcome.reasonCode());
    }

    @Test
    void lookingAtThePlayerStopsAfterTwoSeconds() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.LOOK_AT, null));

        for (int i = 1; i < ActionRun.LOOK_TICKS; i++) {
            ActionRun.Outcome outcome = tick(run, still(2));
            assertFalse(outcome.finished());
            assertEquals(ActionRun.Command.LOOK, outcome.command());
        }

        ActionRun.Outcome outcome = tick(run, still(2));
        assertTrue(outcome.finished());
        assertEquals("done", outcome.status());
    }

    @Test
    void lookingAtSomeoneWhoLeftEndsQuietly() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.LOOK_AT, null));

        ActionRun.Outcome outcome = tick(run, observation(true, false, false, false, 0));

        assertTrue(outcome.finished());
        assertEquals("done", outcome.status());
    }
}
