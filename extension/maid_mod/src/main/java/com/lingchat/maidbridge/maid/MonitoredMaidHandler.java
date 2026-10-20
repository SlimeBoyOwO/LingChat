package com.lingchat.maidbridge.maid;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.bridge.Protocol;
import net.minecraft.server.MinecraftServer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * {@code set_monitored_maid}：记下「只上报这只女仆的事件」。
 *
 * <p>这是玩家聊天能被推送出去的前置条件——没有监控目标，模组就不知道该把谁的聊天转给插件。
 * 换存档时会被清掉，所以插件每次重连都要重发一次。
 *
 * <p>这里故意不校验这只女仆此刻能不能查到：它可能正在未加载的区块里，绑定仍然应该成功。
 */
public class MonitoredMaidHandler implements MessageHandler {
    private static final Logger LOGGER = LoggerFactory.getLogger("lingchat_maid_bridge");

    private static volatile String monitoredMaidId = "";
    /**
     * 她主人的 uuid。她走远到区块卸载、或被 Carry On 抱在怀里时实体查不到，但「主人拿到了
     * 成就」这类全局事件还要认人，所以见到她一次就记下来。
     */
    private static volatile String ownerId = "";

    private final MinecraftServer server;

    public MonitoredMaidHandler(MinecraftServer server) {
        this.server = server;
    }

    public static String get() {
        return monitoredMaidId;
    }

    public static String ownerId() {
        return ownerId;
    }

    /** 见到她时顺手记下主人；主人不会随便换，不需要每次都比对。 */
    public static void observeOwner(EntityMaid maid) {
        if (maid != null && maid.getOwner() != null) {
            ownerId = maid.getOwner().getStringUUID();
        }
    }

    public static void reset() {
        monitoredMaidId = "";
        ownerId = "";
    }

    @Override
    public JsonObject handle(JsonObject request) {
        String next = Envelope.string(Envelope.data(request), "maid_id");
        // 插件会在每次台词上屏时重申一次（它没有定时器，只能借这个时机补发刚连上时
        // 可能丢掉的那一次），所以只在真的换了目标时记一笔，免得日志被刷满。
        if (!next.equals(monitoredMaidId)) {
            LOGGER.info("监控的女仆改为：{}", next.isEmpty() ? "（未指定）" : next);
            // 换了目标，旧主人作废；绑定时她若在已加载区块里，顺手记下新主人
            ownerId = "";
            observeOwner(MaidQueries.findMaidById(server, next));
        }
        monitoredMaidId = next;

        JsonObject data = new JsonObject();
        data.addProperty("monitored_maid_id", monitoredMaidId);
        return Envelope.response(Protocol.CONFIG, data);
    }
}
