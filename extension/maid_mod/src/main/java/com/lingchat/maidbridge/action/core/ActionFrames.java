package com.lingchat.maidbridge.action.core;

import com.google.gson.JsonObject;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.Protocol;

/**
 * 动作相关的帧组装。纯逻辑，只拼 JSON，不碰游戏。
 *
 * <p>{@code accepted=false} 表示受理时就能判定失败（附近没床、玩家不在同一维度、目标超出
 * 居家范围、女仆在睡觉），插件一次往返就知道结果，不留悬案。
 */
public final class ActionFrames {
    private ActionFrames() {
    }

    /** 受理回执。{@code async} 为真表示结果稍后由 {@code action_done} 事件给出。 */
    public static JsonObject accepted(MaidAction action, boolean async, boolean stoodUp, String replacedAction) {
        JsonObject data = base(action);
        data.addProperty("accepted", true);
        data.addProperty("async", async);
        data.addProperty("stood_up", stoodUp);
        data.addProperty("replaced_action", replacedAction == null ? "" : replacedAction);
        data.addProperty("reason_code", "");
        data.addProperty("reason", "");
        return Envelope.response(Protocol.MAID_ACTION_RESULT, data);
    }

    /** 拒绝回执：{@code reason} 是给模型看的中文原因，插件会原样转达。 */
    public static JsonObject rejected(MaidAction action, String reasonCode, String reason) {
        JsonObject data = base(action);
        data.addProperty("accepted", false);
        data.addProperty("async", false);
        data.addProperty("stood_up", false);
        data.addProperty("replaced_action", "");
        data.addProperty("reason_code", reasonCode);
        data.addProperty("reason", reason);
        return Envelope.response(Protocol.MAID_ACTION_RESULT, data);
    }

    /**
     * 动作收尾的事件载荷（由桥接包成 {@code event} 帧广播出去）。
     *
     * <p>三种结局都要推（成功 / 失败 / 被顶掉），否则插件会永远等一个不来的旁白。
     */
    public static JsonObject done(
            String maidId,
            MaidAction action,
            String status,
            String reasonCode,
            String reason,
            int elapsedTicks,
            double distance) {
        JsonObject data = base(action);
        data.addProperty("event_type", "action_done");
        data.addProperty("maid_id", maidId);
        data.addProperty("status", status);
        data.addProperty("reason_code", reasonCode == null ? "" : reasonCode);
        data.addProperty("reason", reason == null ? "" : reason);
        data.addProperty("elapsed_ticks", elapsedTicks);
        // 模型看到的是「还差几格」，不是一串浮点尾巴
        data.addProperty("distance", Math.round(distance * 10.0) / 10.0);
        return data;
    }

    private static JsonObject base(MaidAction action) {
        JsonObject data = new JsonObject();
        data.addProperty("action", action == null ? "" : action.actionId());
        data.addProperty("target", action == null ? "" : action.targetId());
        return data;
    }
}
