package com.lingchat.maidbridge.perception;

import com.google.gson.JsonObject;
import com.lingchat.maidbridge.action.core.ActionFrames;
import com.lingchat.maidbridge.perception.nearby.SurroundingsDiff;

/**
 * 事件帧的公共外形：每条都带 {@code event_type} 与 {@code maid_id}。
 *
 * <p>感知那一层（{@link PerceptionDiff}）与身边那一层（{@link SurroundingsDiff}）各有一套
 * 判定，推出去的帧却是同一套形状，所以外形与取整放在这里，别在两处各抄一份。
 * 动作那一套另有 {@link ActionFrames}。
 */
public final class EventFrames {
    private EventFrames() {
    }

    public static JsonObject base(String eventType, String entityId) {
        JsonObject frame = new JsonObject();
        frame.addProperty("event_type", eventType);
        frame.addProperty("maid_id", entityId);
        return frame;
    }

    /** 距离与血量对模型来说是「大概几格」而不是「3.4999998」，一律留一位小数。 */
    public static double round(double value) {
        return Math.round(value * 10.0) / 10.0;
    }
}
