package com.lingchat.maidbridge.maid;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.bridge.Protocol;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
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
     * 她是不是已经死了。死了之后实体从世界里消失，查不到她；这个标记让「她还活着吗」
     * 不再依赖实体是否加载，也是复活事件的依据。
     */
    private static volatile boolean dead;
    /**
     * 这一局里有没有真的见过她的死活。模组重启后为假：她可能是在上一次游戏里死的，
     * 这时插件该退回台词表里的旁白判断，而不是把「没标记」当成活着。
     */
    private static volatile boolean stateKnown;
    /** 复活时被换下来的旧 uuid。插件若把配置里那个旧 id 再发回来，忽略它。 */
    private static volatile String retiredMaidId = "";
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

    /**
     * 插件请求里点的女仆 id 换成现在这只。
     *
     * <p>胶片不保存 uuid，复活出来的是新实体，而插件配置里存的还是倒下的那一只。
     * 请求一律经过这里，免得复活之后所有指令都「找不到女仆」。
     */
    public static String resolve(String maidId) {
        if (maidId == null) {
            return "";
        }
        if (!retiredMaidId.isEmpty() && retiredMaidId.equals(maidId)) {
            return monitoredMaidId;
        }
        return maidId;
    }

    public static boolean isDead() {
        return dead;
    }

    public static boolean isStateKnown() {
        return stateKnown;
    }

    public static void markDead() {
        dead = true;
        stateKnown = true;
    }

    /** 复活：她换了新的 uuid（胶片不保存 uuid），监控目标跟着改到新的那只。 */
    public static void markAlive(String maidId) {
        dead = false;
        stateKnown = true;
        if (maidId != null && !maidId.isEmpty() && !maidId.equals(monitoredMaidId)) {
            retiredMaidId = monitoredMaidId;
            monitoredMaidId = maidId;
        }
    }

    public static String ownerId() {
        return ownerId;
    }

    /** 见到她时顺手记下主人；主人不会随便换，不需要每次都比对。见到活着的她也算「这一局她活着」。 */
    public static void observeOwner(EntityMaid maid) {
        if (maid == null) {
            return;
        }
        if (maid.getOwner() != null) {
            ownerId = maid.getOwner().getStringUUID();
        }
        if (maid.isAlive()) {
            dead = false;
            stateKnown = true;
        }
    }

    /**
     * 没收到绑定时按说话玩家的女仆兜底。
     *
     * <p>插件只在「连上」与「台词上屏」两个时机补发绑定，而它连上的那一刻她可能还没被
     * 召唤出来（20:26 真机就是：先连上、25 秒后才有女仆）。之后没有绑定就没有聊天帧，
     * 没有聊天帧就没有台词，于是永远等不到下一次补发。这里让第一条玩家发言自己把绑定补上。
     */
    public static void adopt(EntityMaid maid) {
        monitoredMaidId = maid.getStringUUID();
        dead = false;
        stateKnown = true;
        observeOwner(maid);
        LOGGER.info("未收到绑定，按说话玩家的女仆兜底：{}", monitoredMaidId);
    }

    public static void reset() {
        monitoredMaidId = "";
        ownerId = "";
        dead = false;
        stateKnown = false;
        retiredMaidId = "";
    }

    @Override
    public JsonObject handle(JsonObject request) {
        String next = Envelope.string(Envelope.data(request), "maid_id");
        // 插件设置里可能还留着复活前的 uuid：那一只已经不存在了，别把目标改回去
        next = resolve(next);
        // 插件会在每次台词上屏时重申一次（它没有定时器，只能借这个时机补发刚连上时
        // 可能丢掉的那一次），所以只在真的换了目标时记一笔，免得日志被刷满。
        if (!next.equals(monitoredMaidId)) {
            LOGGER.info("监控的女仆改为：{}", next.isEmpty() ? "（未指定）" : next);
            // 换了目标，旧主人作废；绑定时她若在已加载区块里，顺手记下新主人
            ownerId = "";
            dead = false;
            stateKnown = false;
            observeOwner(MaidQueries.findMaidById(server, next));
        }
        // 她死后实体查不到，「她主人是谁」只能从插件给的 MC 用户名认回来。复活时靠它
        // 判断胶片里的是不是同一位主人的女仆。
        String ownerName = Envelope.string(Envelope.data(request), "owner_name");
        if (ownerId.isEmpty() && !ownerName.isEmpty() && server != null) {
            ServerPlayer player = server.getPlayerList().getPlayerByName(ownerName);
            if (player != null) {
                ownerId = player.getStringUUID();
            }
        }
        monitoredMaidId = next;

        JsonObject data = new JsonObject();
        data.addProperty("monitored_maid_id", monitoredMaidId);
        return Envelope.response(Protocol.CONFIG, data);
    }
}
