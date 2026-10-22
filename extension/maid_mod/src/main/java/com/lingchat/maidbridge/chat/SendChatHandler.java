package com.lingchat.maidbridge.chat;

import com.github.tartaricacid.touhoulittlemaid.entity.chatbubble.ChatBubbleManager;
import com.github.tartaricacid.touhoulittlemaid.entity.chatbubble.IChatBubbleData;
import com.github.tartaricacid.touhoulittlemaid.entity.chatbubble.implement.TextChatBubbleData;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.MaidBridgeConfig;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.bridge.Protocol;
import com.lingchat.maidbridge.maid.MaidCondition;
import com.lingchat.maidbridge.maid.MaidQueries;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.ChatFormatting;
import net.minecraft.network.chat.CommonComponents;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.MutableComponent;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;

/**
 * {@code send_chat}：让女仆说一句话。
 *
 * <p>LingChat 里角色的回复逐句发过来，每句都**替换**上一句留下的气泡，而不是往上叠：
 * 车万女仆的气泡集合最多能堆五枚，一句不清就一句叠一句。这一轮只留两枚——台词一枚，
 * 对应情绪的颜文字一枚（候选由插件按情绪给出，随机挑一条在这里做）。
 */
public class SendChatHandler implements MessageHandler {
    private static final int MAX_CHAT_LENGTH = 256;
    /**
     * 颜文字气泡的停留时长（tick，与车万女仆自己的默认值一致）。
     *
     * <p>集合按「到期时刻（毫秒）」排序，渲染时第 0、2、4… 枚画在<b>右边</b>、第 1、3… 枚
     * 画在左边。所以先到期的那枚落在右边：颜文字排前面、台词排后面，看上去就是「台词在左、
     * 颜文字在右」。
     */
    private static final int KAOMOJI_TICKS = IChatBubbleData.DEFAULT_EXIST_TICK;
    /** 台词多停 10 tick：既把两枚的键错开，又让文字比颜文字晚一点消失。 */
    private static final int TEXT_TICKS = KAOMOJI_TICKS + 10;

    private final MinecraftServer server;

    public SendChatHandler(MinecraftServer server) {
        this.server = server;
    }

    @Override
    public JsonObject handle(JsonObject request) {
        if (server == null) {
            return Envelope.error(MaidQueries.NOT_READY);
        }
        JsonObject data = Envelope.data(request);
        String message = sanitize(Envelope.string(data, "message"));
        if (message.isEmpty()) {
            return Envelope.error("消息内容为空");
        }

        // 女仆跑远到区块卸载了就查不到，会走到这里
        EntityMaid maid = MaidQueries.requested(server, request);
        if (maid == null) {
            return Envelope.error(MaidQueries.NOT_FOUND);
        }

        // 昏迷时游戏里保持安静：气泡、聊天栏与转头都停；LingChat 窗口照常显示
        if (MaidCondition.isUnconscious(maid)) {
            JsonObject result = new JsonObject();
            result.addProperty("success", true);
            return Envelope.response(Protocol.CHAT_RESULT, result);
        }

        boolean bubble = MaidBridgeConfig.CHAT_BUBBLE_ENABLED.get();
        boolean chatBox = MaidBridgeConfig.CHAT_BOX_ENABLED.get();

        if (bubble) {
            showBubbles(maid, message, pickKaomoji(data, maid));
        }
        if (chatBox && maid.getOwner() instanceof ServerPlayer owner) {
            owner.sendSystemMessage(chatLine(maid, message));
        }

        // 说话看着对方：只转个头，不占动作槽、也不给模型任何回执；打架、干活、睡觉时不打扰。
        MaidActuator.faceOwnerWhileTalking(maid);

        JsonObject result = new JsonObject();
        result.addProperty("success", true);
        return Envelope.response(Protocol.CHAT_RESULT, result);
    }

    /**
     * 只留这一句的两枚气泡：先清掉上一句的，再放台词与颜文字。
     *
     * <p>车万女仆没有「替换」入口：它的集合按到期时刻做键、最多叠五枚，只能先清空再重新
     * 加（{@code addChatBubble} 会顺带把新集合同步给客户端）。
     */
    private static void showBubbles(EntityMaid maid, String message, String kaomoji) {
        ChatBubbleManager bubbles = maid.getChatBubbleManager();
        bubbles.getChatBubbleDataCollection().chatBubbles().clear();
        bubbles.addChatBubble(TextChatBubbleData.create(
                TEXT_TICKS, Component.literal(message), IChatBubbleData.TYPE_2, IChatBubbleData.DEFAULT_PRIORITY));
        if (!kaomoji.isEmpty()) {
            bubbles.addChatBubble(TextChatBubbleData.create(
                    KAOMOJI_TICKS, Component.literal(kaomoji), IChatBubbleData.TYPE_2, IChatBubbleData.DEFAULT_PRIORITY));
        }
    }

    /**
     * 从插件给的候选里随机挑一条颜文字。
     *
     * <p>挑哪一条交给模组是为了用游戏自己的随机源；插件只负责「什么情绪配哪些颜文字」
     * 这类中文措辞。没有候选（老插件、认不出的情绪）就只显示台词气泡。
     */
    private static String pickKaomoji(JsonObject data, EntityMaid maid) {
        JsonElement element = data.get("kaomoji");
        if (element == null || !element.isJsonArray()) {
            return "";
        }
        JsonArray array = element.getAsJsonArray();
        List<String> candidates = new ArrayList<>(array.size());
        for (JsonElement item : array) {
            if (item.isJsonPrimitive()) {
                String text = sanitize(item.getAsString());
                if (!text.isEmpty()) {
                    candidates.add(text);
                }
            }
        }
        if (candidates.isEmpty()) {
            return "";
        }
        return candidates.get(maid.getRandom().nextInt(candidates.size()));
    }

    private static MutableComponent chatLine(EntityMaid maid, String message) {
        MutableComponent line = Component.literal("<")
                .append(maid.getName())
                .append(Component.literal(">"))
                .append(CommonComponents.SPACE)
                .append(message);
        return line.withStyle(ChatFormatting.GRAY);
    }

    /**
     * 气泡和聊天栏都容不下换行，而且端口就开在本机、任何本地进程都能连上来发话，
     * 所以顺手把换行压平、把长度截断，免得被刷屏。
     */
    private static String sanitize(String raw) {
        String flat = raw.replace('\n', ' ').replace('\r', ' ').trim();
        return flat.length() > MAX_CHAT_LENGTH ? flat.substring(0, MAX_CHAT_LENGTH) : flat;
    }
}
