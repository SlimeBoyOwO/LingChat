package com.lingchat.maidbridge.action.core;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.Test;

/**
 * 动作帧的解析：这部分是纯逻辑，能脱开游戏跑。
 *
 * <p>要守住的是「白名单与归一」：认不出的动作要当场拒绝，认不出的目标要退回默认，
 * 不带目标的动作不许把目标带上——端口没鉴权，任何本地进程都能连上来发帧。
 */
class MaidActionTest {

    private static JsonObject data(String action, String target) {
        JsonObject data = new JsonObject();
        data.addProperty("maid_id", "maid-uuid-1");
        data.addProperty("action", action);
        if (target != null) {
            data.addProperty("target", target);
        }
        return data;
    }

    private static MaidAction parse(String action, String target) {
        MaidAction.ParseResult parsed = MaidAction.parse(data(action, target));
        assertTrue(parsed.accepted(), () -> "本该受理：" + action + "/" + target + " → " + parsed.error());
        assertEquals("maid-uuid-1", parsed.maidId());
        return parsed.action();
    }

    @Test
    void knownActionsAreAcceptedWithDefaultTargets() {
        assertEquals(MaidAction.Kind.GO_TO, parse("go_to", null).kind());
        assertEquals(MaidAction.Target.PLAYER, parse("go_to", null).target());
        assertEquals(MaidAction.Kind.SIT, parse("sit", null).kind());
        assertNull(parse("sit", null).target());
    }

    @Test
    void actionAndTargetAreNormalized() {
        assertEquals(MaidAction.Kind.GO_TO, parse(" GO_TO ", null).kind());
        assertEquals(MaidAction.Target.BED, parse("go_to", " BED ").target());
        assertEquals(MaidAction.Target.BED, parse("LIE_DOWN", null).target(), "省略目标时躺下默认去床边");
        assertEquals(MaidAction.Target.BED, parse("lie_down", "player").target(), "躺下只认床");
        assertEquals(MaidAction.Target.PLAYER, parse("go_to", "chair").target(), "认不出的目标退回默认");
    }

    @Test
    void actionsWithoutTargetDoNotCarryOne() {
        assertNull(parse("sit", "bed").target());
        assertNull(parse("stand", "player").target());
        assertNull(parse("look_at", "bed").target());
        assertEquals("", parse("sit", "bed").targetId());
    }

    @Test
    void unknownActionAndMissingMaidAreRejected() {
        MaidAction.ParseResult unknown = MaidAction.parse(data("fly", null));
        assertFalse(unknown.accepted());
        assertEquals("unsupported_action", unknown.errorCode());
        assertTrue(unknown.error().contains("fly"));

        JsonObject noMaid = new JsonObject();
        noMaid.addProperty("action", "sit");
        MaidAction.ParseResult missing = MaidAction.parse(noMaid);
        assertFalse(missing.accepted());
        assertEquals("missing_maid_id", missing.errorCode());
    }

    /** 写反了插件就会永远等一个不来的收尾：只有耗时动作才有完成事件。 */
    @Test
    void onlySlowActionsHaveCompletionEvents() {
        assertTrue(parse("go_to", "player").hasCompletionEvent());
        assertTrue(parse("go_to", "bed").hasCompletionEvent());
        assertTrue(parse("lie_down", null).hasCompletionEvent());
        assertFalse(parse("sit", null).hasCompletionEvent());
        assertFalse(parse("stand", null).hasCompletionEvent());
        assertFalse(parse("look_at", null).hasCompletionEvent());
    }

    @Test
    void arrivalDistanceMatchesTheSinkCloseEnoughDistance() {
        assertEquals(3, parse("go_to", "player").arriveDistance());
        assertEquals(2, parse("go_to", "bed").arriveDistance());
        assertEquals(2, parse("lie_down", null).arriveDistance());
    }

    @Test
    void onlyWalkingActionsAreRestrictedByHomeRadius() {
        assertTrue(parse("go_to", "player").walks());
        assertTrue(parse("lie_down", null).walks());
        assertFalse(parse("sit", null).walks());
        assertFalse(parse("look_at", null).walks());
    }
}
