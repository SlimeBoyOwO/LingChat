package com.lingchat.maidbridge.bridge;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

/** 路由层里冒烟测试覆盖不到的三件事：线程契约、request_id 回填、失败兜底。 */
class MessageRouterTest {
    private static final int MAX_QUEUE_SIZE = 500;
    private static final int MAX_TASKS_PER_TICK = 50;

    private final List<JsonObject> replies = new ArrayList<>();
    private final MessageRouter router = new MessageRouter();

    private void route(String type, String requestJson) {
        router.route(type, JsonParser.parseString(requestJson).getAsJsonObject(), replies::add);
    }

    /** 网络线程只入队；碰游戏世界的活必须留到 tick 里做。 */
    @Test
    void handlerRunsOnTickNotOnTheCallingThread() {
        boolean[] ran = {false};
        router.register("work", request -> {
            ran[0] = true;
            return Envelope.response("done", new JsonObject());
        });

        route("work", "{\"type\":\"work\",\"request_id\":\"r1\"}");
        assertFalse(ran[0], "handler 不应在网络线程上执行");

        router.tick();
        assertTrue(ran[0]);
        assertEquals("r1", Envelope.requestId(replies.get(0)));
    }

    /** 单向通知（比如 set_monitored_maid）没有 request_id，回填必须是空操作。 */
    @Test
    void requestIdIsFilledInOnlyWhenTheRequestHadOne() {
        router.register("echo", request -> Envelope.response("echoed", new JsonObject()));

        route("echo", "{\"type\":\"echo\",\"request_id\":\"r2\"}");
        route("echo", "{\"type\":\"echo\"}");
        router.tick();

        assertEquals("r2", Envelope.requestId(replies.get(0)));
        assertFalse(replies.get(1).has("request_id"));
    }

    /** 车万女仆换版本后 API 漂移抛的是 NoSuchMethodError，那是 Error，漏了会打断整个 tick。 */
    @Test
    void handlerFailureBecomesAnErrorFrame() {
        router.register("boom", request -> {
            throw new IllegalStateException("炸了");
        });
        router.register("drift", request -> {
            throw new NoSuchMethodError("TLM 的 API 变了");
        });

        route("boom", "{\"type\":\"boom\",\"request_id\":\"r3\"}");
        route("drift", "{\"type\":\"drift\",\"request_id\":\"r4\"}");
        router.tick();

        assertEquals(Protocol.ERROR, Envelope.type(replies.get(0)));
        assertEquals("r3", Envelope.requestId(replies.get(0)));
        assertEquals(Protocol.ERROR, Envelope.type(replies.get(1)));
    }

    /** 两个安全阀：队列有上限，单 tick 有处理上限，都不能把主线程拖垮。 */
    @Test
    void queueAndTickAreRateLimited() {
        int[] ran = {0};
        router.register("fill", request -> {
            ran[0]++;
            return null;
        });

        for (int i = 0; i < MAX_QUEUE_SIZE + 1; i++) {
            route("fill", "{\"type\":\"fill\",\"request_id\":\"r" + i + "\"}");
        }
        assertEquals(1, replies.size(), "超出队列上限的那条应立刻回错，而不是静默丢弃");
        assertEquals(Protocol.ERROR, Envelope.type(replies.get(0)));

        router.tick();
        assertEquals(MAX_TASKS_PER_TICK, ran[0]);

        for (int i = 0; i < 20 && ran[0] < MAX_QUEUE_SIZE; i++) {
            router.tick();
        }
        assertEquals(MAX_QUEUE_SIZE, ran[0], "队列里剩下的都该被执行完");
    }
}
