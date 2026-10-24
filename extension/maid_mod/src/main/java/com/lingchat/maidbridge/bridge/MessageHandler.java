package com.lingchat.maidbridge.bridge;

import com.google.gson.JsonObject;

/**
 * 一条请求的处理者。返回 null 表示这条请求不需要回帧。
 *
 * <p>约定：实现里可以直接读改游戏世界，因为 {@link MessageRouter} 保证它在服务端主线程上被调用。
 */
public interface MessageHandler {
    JsonObject handle(JsonObject request);
}
