package com.lingchat.maidbridge.bridge;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.net.ServerSocket;
import java.net.URI;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import org.java_websocket.client.WebSocketClient;
import org.java_websocket.handshake.ServerHandshake;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

/**
 * 冒烟测试：真的起一个 WebSocket 服务端，真的连上去，按协议收发几帧。
 *
 * <p>覆盖的是 PC 之外最容易出错的那一层——信封形状与回帧规则。读改游戏世界的那部分
 * 这里碰不到，只能进游戏验。
 */
class BridgeSmokeTest {
    private static final long TIMEOUT_SECONDS = 5;

    private final BlockingQueue<String> received = new LinkedBlockingQueue<>();
    private final MessageRouter router = new MessageRouter();

    private BridgeServer server;
    private WebSocketClient client;
    private Thread ticker;

    @BeforeEach
    void startServer() throws Exception {
        router.register("echo", request -> {
            JsonObject data = new JsonObject();
            data.addProperty("回显", Envelope.string(Envelope.data(request), "message"));
            return Envelope.response("echo_result", data);
        });

        try (ServerSocket probe = new ServerSocket(0)) {
            server = new BridgeServer(probe.getLocalPort(), router);
        }
        server.start();

        // 用游戏里的 tick 驱动队列：真实环境是 ServerTickEvent.Post，这里是 10ms 一轮
        ticker = new Thread(() -> {
            while (!Thread.currentThread().isInterrupted()) {
                router.tick();
                try {
                    Thread.sleep(10);
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                }
            }
        });
        ticker.setDaemon(true);
        ticker.start();

        client = connect(server.getPort());
    }

    @AfterEach
    void stopServer() throws Exception {
        ticker.interrupt();
        if (client != null) {
            client.closeBlocking();
        }
        if (server != null) {
            server.stop(500);
        }
    }

    @Test
    void pingIsAnsweredWithPong() throws Exception {
        send("{\"type\":\"ping\",\"request_id\":\"t1\"}");

        JsonObject pong = receive();
        assertEquals(Protocol.PONG, Envelope.type(pong));
        assertEquals("t1", Envelope.requestId(pong));
    }

    @Test
    void pingWithoutRequestIdComesBackWithoutOne() throws Exception {
        send("{\"type\":\"ping\"}");

        assertFalse(receive().has("request_id"));
    }

    @Test
    void registeredHandlerRepliesAcrossTheSocket() throws Exception {
        send("{\"type\":\"echo\",\"request_id\":\"t2\",\"data\":{\"message\":\"晚上好\"}}");

        JsonObject reply = receive();
        assertEquals("echo_result", Envelope.type(reply));
        assertEquals("t2", Envelope.requestId(reply));
        assertEquals("晚上好", Envelope.string(Envelope.data(reply), "回显"));
    }

    @Test
    void unknownTypeRepliesWithError() throws Exception {
        send("{\"type\":\"没有这个类型\",\"request_id\":\"t3\"}");

        JsonObject reply = receive();
        assertEquals(Protocol.ERROR, Envelope.type(reply));
        assertEquals("t3", Envelope.requestId(reply));
    }

    @Test
    void malformedFramesReplyWithErrorAndKeepTheConnectionAlive() throws Exception {
        send("这不是 JSON");
        assertEquals(Protocol.ERROR, Envelope.type(receive()));

        send("{\"没有类型字段\":1}");
        assertEquals(Protocol.ERROR, Envelope.type(receive()));

        // 连接还在：再来一条正常请求仍然通
        send("{\"type\":\"ping\",\"request_id\":\"t4\"}");
        assertEquals("t4", Envelope.requestId(receive()));
    }

    /**
     * 推送帧绝不能带 request_id：LingChat 宿主按它认领响应，带上就会被当成某个请求的
     * 应答吞掉，玩家在 MC 里说的话就永远进不了对话。
     */
    @Test
    void pushedChatMessageCarriesNoRequestId() throws Exception {
        JsonObject chat = new JsonObject();
        chat.addProperty("event_type", "chat");
        chat.addProperty("sender", "Steve");
        chat.addProperty("message", "在吗");

        server.broadcastChatMessage(chat);

        JsonObject frame = receive();
        assertEquals(Protocol.CHAT_MESSAGE, Envelope.type(frame));
        assertFalse(frame.has("request_id"));
        assertEquals("Steve", Envelope.string(Envelope.data(frame), "sender"));
    }

    private void send(String frame) {
        client.send(frame);
    }

    private JsonObject receive() throws InterruptedException {
        String frame = received.poll(TIMEOUT_SECONDS, TimeUnit.SECONDS);
        assertNotNull(frame, "等不到模组的回帧");
        return JsonParser.parseString(frame).getAsJsonObject();
    }

    private WebSocketClient connect(int port) throws Exception {
        URI uri = URI.create("ws://127.0.0.1:" + port);
        for (int attempt = 0; attempt < 50; attempt++) {
            WebSocketClient candidate = new WebSocketClient(uri) {
                @Override
                public void onOpen(ServerHandshake handshake) {
                }

                @Override
                public void onMessage(String message) {
                    received.add(message);
                }

                @Override
                public void onClose(int code, String reason, boolean remote) {
                }

                @Override
                public void onError(Exception ex) {
                }
            };
            if (candidate.connectBlocking(1, TimeUnit.SECONDS)) {
                assertTrue(candidate.isOpen());
                return candidate;
            }
            candidate.closeBlocking();
        }
        throw new IllegalStateException("连不上刚起的桥接服务端");
    }
}
