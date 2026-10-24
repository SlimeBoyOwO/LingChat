package com.lingchat.maidbridge.bridge;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.List;

/**
 * 消息信封的读写。所有帧都是 {@code {"type":..,"request_id":..,"data":{..}}}，
 * {@code request_id} 只出现在请求与它的响应上，推送帧绝不能带——LingChat 宿主按它
 * 认领响应，误带会让推送帧被当成某个请求的应答吞掉。
 *
 * <p>回填 {@code request_id} 是路由层的统一责任（见 {@link #withRequestId}），
 * handler 只管拼自己的业务帧。这里刻意不碰任何 Minecraft 类，是整套协议里唯一
 * 能脱离游戏跑单测的部分。
 */
public final class Envelope {
    private Envelope() {
    }

    /** 缺字段或类型不对一律当空串，调用方按空串判断即可，不用到处判 null。 */
    public static String string(JsonObject obj, String key) {
        if (obj == null) {
            return "";
        }
        JsonElement element = obj.get(key);
        if (element == null || !element.isJsonPrimitive()) {
            return "";
        }
        return element.getAsString().trim();
    }

    /** 取一个字符串数组；缺字段、类型不对、非字符串的元素一律跳过。 */
    public static List<String> strings(JsonObject obj, String key) {
        List<String> result = new ArrayList<>();
        if (obj == null) {
            return result;
        }
        JsonElement element = obj.get(key);
        if (element == null || !element.isJsonArray()) {
            return result;
        }
        for (JsonElement one : element.getAsJsonArray()) {
            if (one.isJsonPrimitive()) {
                result.add(one.getAsString());
            }
        }
        return result;
    }

    public static String type(JsonObject frame) {
        return string(frame, "type");
    }

    public static String requestId(JsonObject frame) {
        return string(frame, "request_id");
    }

    /** 取业务负载；没有或不是对象时给一个空对象，免得每个 handler 都判一次。 */
    public static JsonObject data(JsonObject frame) {
        if (frame == null) {
            return new JsonObject();
        }
        JsonElement element = frame.get("data");
        return element != null && element.isJsonObject() ? element.getAsJsonObject() : new JsonObject();
    }

    public static JsonObject response(String type, JsonObject data) {
        JsonObject frame = new JsonObject();
        frame.addProperty("type", type);
        if (data != null) {
            frame.add("data", data);
        }
        return frame;
    }

    public static JsonObject error(String message) {
        JsonObject data = new JsonObject();
        data.addProperty("message", message);
        return response(Protocol.ERROR, data);
    }

    public static JsonObject pong() {
        return response(Protocol.PONG, null);
    }

    /** 把请求的 {@code request_id} 原样回填到响应上；单向通知没有 id，回填就是空操作。 */
    public static JsonObject withRequestId(JsonObject frame, String requestId) {
        if (frame != null && requestId != null && !requestId.isEmpty() && !frame.has("request_id")) {
            frame.addProperty("request_id", requestId);
        }
        return frame;
    }
}
