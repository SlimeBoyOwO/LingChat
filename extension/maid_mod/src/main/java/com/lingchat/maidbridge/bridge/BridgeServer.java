package com.lingchat.maidbridge.bridge;

import com.google.gson.Gson;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.util.Collections;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import org.java_websocket.WebSocket;
import org.java_websocket.handshake.ClientHandshake;
import org.java_websocket.server.WebSocketServer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * 只监听本机回环的 WebSocket 服务端。
 *
 * <p>收帧后除了 {@code ping} 这种不碰游戏状态的，一律交给 {@link MessageRouter} 排队，
 * 本类自己绝不读改游戏世界。
 */
public class BridgeServer extends WebSocketServer {
    private static final Logger LOGGER = LoggerFactory.getLogger("lingchat_maid_bridge");
    private static final Gson GSON = new Gson();

    private final Set<WebSocket> clients = Collections.newSetFromMap(new ConcurrentHashMap<>());
    private final MessageRouter router;

    public BridgeServer(int port, MessageRouter router) {
        super(new InetSocketAddress("127.0.0.1", port));
        this.router = router;
        setReuseAddr(true);
    }

    @Override
    public void onOpen(WebSocket conn, ClientHandshake handshake) {
        InetAddress address = conn.getRemoteSocketAddress() instanceof InetSocketAddress socketAddress
                ? socketAddress.getAddress()
                : null;
        if (address == null || !address.isLoopbackAddress()) {
            LOGGER.warn("拒绝非本机连接：{}", address);
            conn.close(403, "Only localhost connections allowed");
            return;
        }
        clients.add(conn);
        LOGGER.info("LingChat 已连上桥接");
    }

    @Override
    public void onClose(WebSocket conn, int code, String reason, boolean remote) {
        clients.remove(conn);
        LOGGER.info("LingChat 断开连接：code={} reason={}", code, reason);
    }

    @Override
    public void onMessage(WebSocket conn, String message) {
        JsonObject frame;
        try {
            JsonElement parsed = JsonParser.parseString(message);
            if (!parsed.isJsonObject()) {
                send(conn, Envelope.error("消息必须是 JSON 对象"));
                return;
            }
            frame = parsed.getAsJsonObject();
        } catch (Exception e) {
            send(conn, Envelope.error("消息不是合法 JSON"));
            return;
        }

        String type = Envelope.type(frame);
        if (type.isEmpty()) {
            send(conn, Envelope.error("消息缺少 type 字段"));
            return;
        }
        // 心跳不回主线程，游戏卡住时也要能应答，否则插件那边分不清是卡了还是断了。
        if (Protocol.PING.equals(type)) {
            send(conn, Envelope.withRequestId(Envelope.pong(), Envelope.requestId(frame)));
            return;
        }
        router.route(type, frame, response -> send(conn, response));
    }

    /** 只收文本帧；二进制帧一律拒绝。 */
    @Override
    public void onMessage(WebSocket conn, java.nio.ByteBuffer bytes) {
        conn.close(1003, "Text frames only");
    }

    @Override
    public void onError(WebSocket conn, Exception ex) {
        LOGGER.warn("桥接连接出错：{}", ex.getMessage());
    }

    @Override
    public void onStart() {
        LOGGER.info("桥接已启动：ws://127.0.0.1:{}", getPort());
    }

    /** 玩家在游戏里说话时推给插件。 */
    public void broadcastChatMessage(JsonObject chatData) {
        broadcast(Protocol.CHAT_MESSAGE, chatData);
    }

    /** 游戏世界的事件推给插件（换地形、天黑、下雨、受伤这类）。 */
    public void broadcastEvent(JsonObject eventData) {
        broadcast(Protocol.EVENT, eventData);
    }

    /**
     * 自发推送的出口：广播给所有已连接客户端。
     *
     * <p>推送帧**绝不能**带 {@code request_id}——LingChat 宿主按它认领响应，
     * 误带会让这一帧被当成某个请求的应答吞掉（见 {@link Envelope} 的说明）。
     */
    private void broadcast(String type, JsonObject data) {
        if (clients.isEmpty()) {
            return;
        }
        String json = GSON.toJson(Envelope.response(type, data));
        for (WebSocket client : clients) {
            if (client.isOpen()) {
                try {
                    client.send(json);
                } catch (Exception e) {
                    LOGGER.warn("推送 {} 失败：{}", type, e.getMessage());
                }
            }
        }
    }

    public boolean hasClients() {
        return !clients.isEmpty();
    }

    private void send(WebSocket conn, JsonObject frame) {
        if (conn == null || !conn.isOpen()) {
            return;
        }
        try {
            conn.send(GSON.toJson(frame));
        } catch (Exception e) {
            LOGGER.warn("发送响应失败：{}", e.getMessage());
        }
    }
}
