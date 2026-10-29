package com.lingchat.maidbridge.action.core;

import com.google.gson.JsonObject;
import com.lingchat.maidbridge.action.WanderGuard;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.bridge.Protocol;

/**
 * {@code maid_settle}：插件这一轮说完话了，动作之后按住的随机游荡可以放开。
 *
 * <p>不发这个帧也能用：模组那边最多再等十秒就自己放开，主人走远时也会立刻放开。插件只在
 * 每轮最后一句上屏时发一次，重复发没有副作用。动作还在跑时这一帧不作数——那时说完了不等于
 * 事情办完了，收尾之后照样再从十秒开始倒数。
 */
public class MaidSettleHandler implements MessageHandler {
    @Override
    public JsonObject handle(JsonObject request) {
        String maidId = Envelope.string(Envelope.data(request), "maid_id");
        if (maidId.isEmpty()) {
            return Envelope.error("没有指定女仆（maid_id 为空）");
        }
        WanderGuard.spoken(maidId);
        JsonObject data = new JsonObject();
        data.addProperty("maid_id", maidId);
        return Envelope.response(Protocol.MAID_SETTLE_RESULT, data);
    }
}
