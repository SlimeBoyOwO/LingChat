package com.lingchat.maidbridge.action.core;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import org.junit.jupiter.api.Test;

/**
 * 动作状态机：这部分是纯逻辑，能脱开游戏跑。
 *
 * <p>要守住的是「什么时候算成、什么时候如实认输」：到位阈值两侧、卡在床边、走不过去、
 * 躺下之后被打断、看向玩家到时收尾，以及「她不见了」优先于其他一切判定。
 */
class ActionRunTest {

    private static MaidAction action(MaidAction.Kind kind, MaidAction.Target target) {
        return new MaidAction(kind, target);
    }

    private static ActionRun.Observation still(int distance) {
        return new ActionRun.Observation(
                true, false, false, true, false, distance, distance, ActionRun.Interrupt.NONE);
    }

    private static ActionRun.Observation observation(
            boolean maidPresent, boolean blocked, boolean sleeping, boolean targetPresent, int distance) {
        return new ActionRun.Observation(
                maidPresent, blocked, sleeping, targetPresent, false, distance, distance,
                ActionRun.Interrupt.NONE);
    }

    private static ActionRun.Observation holding(ActionRun.Interrupt interrupt) {
        return new ActionRun.Observation(true, false, false, true, false, 0, 0.0, interrupt);
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
                new ActionRun.Observation(
                        true, false, false, true, true, 10, 10.0, ActionRun.Interrupt.NONE);

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
                new ActionRun.Observation(
                        true, false, false, true, true, 10, 10.0, ActionRun.Interrupt.NONE);

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

        assertFalse(outcome.finished(), "贴到床边只是走到，接下来要躺下");
        assertEquals(ActionRun.Command.LIE, outcome.command());
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

        ActionRun.Outcome outcome = tick(run, new ActionRun.Observation(false, true, true, false, true, 99, 99, ActionRun.Interrupt.NONE));

        assertEquals("failed", outcome.status());
        assertEquals("maid_gone", outcome.reasonCode());
    }

    /** 床上躺下和就地躺下一样：到了就不再收尾，一直躺着直到被打断。 */
    @Test
    void lyingDownWalksToTheBedThenHoldsThePose() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.LIE_DOWN, MaidAction.Target.BED));

        assertEquals(ActionRun.Command.WALK, tick(run, still(6)).command());

        ActionRun.Outcome reachedBed = tick(run, still(2));
        assertFalse(reachedBed.finished(), "贴到床边只是走到，还没躺下");
        assertEquals(ActionRun.Command.LIE, reachedBed.command());
        assertTrue(run.lying());

        ActionRun.Observation asleep = observation(true, true, true, true, 1);
        for (int i = 0; i < 400; i++) {
            ActionRun.Outcome outcome = tick(run, asleep);
            assertFalse(outcome.finished());
            assertEquals(ActionRun.Command.LIE, outcome.command());
        }
    }

    /** 床上被叫醒也要如实说是怎么醒的，而不是当成「没做成」。 */
    @Test
    void wakingUpInTheBedEndsTheRunWithTheReason() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.LIE_DOWN, MaidAction.Target.BED));
        tick(run, still(2));

        ActionRun.Outcome outcome =
                tick(run, holding(new ActionRun.Interrupt("woken", "自己爬了起来")));

        assertTrue(outcome.finished());
        assertEquals("done", outcome.status());
        assertEquals("woken", outcome.reasonCode());
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

    /** 就地躺下没有时长也没有到位线：不被打断就一直躺着。 */
    @Test
    void lyingOnTheGroundHoldsUntilSomethingInterrupts() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.LIE_DOWN, MaidAction.Target.GROUND));

        for (int i = 0; i < 400; i++) {
            ActionRun.Outcome outcome = tick(run, holding(ActionRun.Interrupt.NONE));
            assertFalse(outcome.finished());
            assertEquals(ActionRun.Command.LIE, outcome.command());
        }
    }

    /** 探路是持续状态：没有时长也没有到位线，不被打断就一直跟着主人换点。 */
    @Test
    void scoutingHoldsUntilSomethingStopsIt() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.SCOUT, null));

        for (int i = 0; i < 400; i++) {
            ActionRun.Outcome outcome = tick(run, holding(ActionRun.Interrupt.NONE));
            assertFalse(outcome.finished());
            assertEquals(ActionRun.Command.SCOUT, outcome.command());
        }
    }

    /** 血不多、被抱走、跟主人走散都按「自己停下来」收尾，并且要说是哪件事。 */
    @Test
    void scoutingStopsWhenSheCannotKeepItUp() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.SCOUT, null));
        tick(run, holding(ActionRun.Interrupt.NONE));

        ActionRun.Outcome outcome =
                tick(run, holding(new ActionRun.Interrupt("low_health", "血不多了")));

        assertTrue(outcome.finished());
        assertEquals("done", outcome.status());
        assertEquals("low_health", outcome.reasonCode());
        assertEquals("血不多了", outcome.reason());
    }

    /** 她躺不住时收尾要如实说是哪件事让她起来的，而不是当成失败。 */
    @Test
    void gettingUpEndsTheLyingRunWithTheReason() {
        ActionRun run = new ActionRun(action(MaidAction.Kind.LIE_DOWN, MaidAction.Target.GROUND));
        tick(run, holding(ActionRun.Interrupt.NONE));

        ActionRun.Outcome outcome = tick(run, holding(new ActionRun.Interrupt("hurt", "挨了一下")));

        assertTrue(outcome.finished());
        assertEquals("done", outcome.status());
        assertEquals("hurt", outcome.reasonCode());
        assertEquals("挨了一下", outcome.reason());
    }
}
