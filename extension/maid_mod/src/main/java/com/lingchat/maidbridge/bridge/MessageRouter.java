package com.lingchat.maidbridge.bridge;

import com.google.gson.JsonObject;
import java.util.HashMap;
import java.util.Map;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.function.Consumer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * 把网络线程收到的请求搬到服务端主线程上执行。
 *
 * <p>WebSocket 的回调跑在它自己的线程里，而读女仆、发气泡、加气泡都要碰游戏世界，
 * 只能在主线程做。所以这里只做入队，真正的处理留给每 tick 的 {@link #tick()}。
 *
 * <p>也是单人游戏按 Esc 暂停时请求会超时的原因：tick 停了，队列就不再往前走。
 */
public class MessageRouter {
    private static final Logger LOGGER = LoggerFactory.getLogger("lingchat_maid_bridge");
    private static final int MAX_QUEUE_SIZE = 500;
    private static final int MAX_TASKS_PER_TICK = 50;

    private final Map<String, MessageHandler> handlers = new HashMap<>();
    private final ConcurrentLinkedQueue<Runnable> mainThreadTasks = new ConcurrentLinkedQueue<>();

    public void register(String type, MessageHandler handler) {
        handlers.put(type, handler);
    }

    /**
     * 在网络线程上调用：只做分发与入队。
     *
     * @param reply 回帧的出口，由调用方绑定到具体连接上
     */
    public void route(String type, JsonObject request, Consumer<JsonObject> reply) {
        String requestId = Envelope.requestId(request);
        // 统一在这里回填 request_id，handler 就不用各自记得处理
        Consumer<JsonObject> respond = frame -> reply.accept(Envelope.withRequestId(frame, requestId));

        MessageHandler handler = handlers.get(type);
        if (handler == null) {
            respond.accept(Envelope.error("不支持的消息类型：" + type));
            return;
        }
        if (mainThreadTasks.size() >= MAX_QUEUE_SIZE) {
            LOGGER.warn("主线程任务队列已满（{}），丢弃消息 type={}", MAX_QUEUE_SIZE, type);
            respond.accept(Envelope.error("服务器繁忙，请稍后重试"));
            return;
        }
        mainThreadTasks.add(() -> {
            try {
                JsonObject response = handler.handle(request);
                if (response != null) {
                    respond.accept(response);
                }
            } catch (Throwable e) {
                // 捕 Throwable 而不是 Exception：车万女仆换版本后 API 漂移的典型表现是
                // NoSuchMethodError / NoClassDefFoundError，那是 Error。漏掉就会从 tick
                // 回调里冒出去打断整个 tick。
                LOGGER.error("处理消息失败 type={}", type, e);
                respond.accept(Envelope.error("内部错误：" + e.getMessage()));
            }
        });
    }

    /** 在服务端主线程的 tick 里调用。单 tick 限量执行，避免刷爆主线程。 */
    public void tick() {
        int processed = 0;
        Runnable task;
        while (processed < MAX_TASKS_PER_TICK && (task = mainThreadTasks.poll()) != null) {
            try {
                task.run();
            } catch (Throwable e) {
                LOGGER.error("主线程任务执行失败", e);
            }
            processed++;
        }
    }
}
