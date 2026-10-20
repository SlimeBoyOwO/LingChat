package com.lingchat.maidbridge.perception.carry;

import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.MaidBridgeConfig;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.integration.carry.CarryOnBridge;
import com.lingchat.maidbridge.integration.carry.CarryOnPresent;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.maid.MonitoredMaidHandler;
import com.lingchat.maidbridge.perception.EventFrames;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;

/**
 * 她被抱起来 / 被放下来。
 *
 * <p>两条判定并用：
 *
 * <ul>
 *   <li>**Carry On 自己的事件**（{@link com.lingchat.maidbridge.integration.carry.CarryOnEvents}）：
 *       抱起时事件先于「把她移出世界」触发，放回时 {@code EntityJoinLevelEvent} 触发。
 *       这是主路径。</li>
 *   <li>**每 tick 翻玩家的 carry NBT**：事件没挂上（Carry On 版本太老、注册失败）时的兜底。
 *       Carry On 抱人时会把实体 NBT 存在玩家身上，挨个玩家问「这份 NBT 的 UUID 是不是她」。</li>
 * </ul>
 *
 * <p>只在有客户端连着、绑了女仆、且感知开着时上报；断线期间发生的变化不补报，重连后
 * 如果她还被抱着，会按「当前状态」再报一次，插件由此知道她此刻在谁怀里。
 */
public final class CarryReporter {
    private static final org.slf4j.Logger LOGGER =
            org.slf4j.LoggerFactory.getLogger(LingChatMaidBridge.MOD_ID);

    /** 上一 tick 她是不是被抱着。 */
    private static boolean carried;
    /** 上一 tick 盯的是哪只女仆；换了目标要把状态归零。 */
    private static String watchedMaid = "";
    /** 这次「被抱着」是 Carry On 事件告诉我们的：是的话就别再用 NBT 扫描覆盖它。 */
    private static boolean viaEvent;
    /** 反复抱起放下不该每次都被她「感知」：每种事件每个监控周期最多报两条。 */
    private static final int MAX_REPORTS = 2;
    private static int carriedReports;
    private static int droppedReports;

    private CarryReporter() {
    }

    /**
     * 每个服务端 tick 调一次（主线程）。
     *
     * <p>事件路径一旦接过手（{@code viaEvent}），这里只做一件兜底的事：她重新出现在世界里
     * 却没收着「放下了」的 join 事件（老版本、事件被别的模组取消之类），就补一条放下。
     */
    public static void tick(MinecraftServer server) {
        if (!watching()) {
            reset();
            return;
        }
        String maidId = MonitoredMaidHandler.get();
        if (!maidId.equals(watchedMaid)) {
            watchedMaid = maidId;
            carried = false;
            viaEvent = false;
            carriedReports = 0;
            droppedReports = 0;
        }
        if (viaEvent) {
            // 她回到世界里了，说明放下那次 join 事件没收到：补一条，别让她一直「在怀里」
            if (MaidQueries.findMaidById(server, maidId) != null) {
                markDropped(maidId);
            }
            return;
        }
        ServerPlayer carrier = carrierOf(server, maidId);
        if (carrier != null && !carried) {
            carried = true;
            reportCarried(maidId);
        } else if (carrier == null && carried) {
            markDropped(maidId);
        }
    }

    /**
     * Carry On 的抱起事件：在实体被移出世界之前发。
     *
     * <p>事件不管有没有客户端都会记下「她正被抱着」——但没客户端时不能把它标成事件路径，
     * 否则重连后就没有人再报这一条了。
     */
    public static void pickedUp(String maidId, String carrierId) {
        if (maidId == null || maidId.isEmpty() || !maidId.equals(MonitoredMaidHandler.get())) {
            return;
        }
        if (carried) {
            return;
        }
        watchedMaid = maidId;
        carried = true;
        if (!watching()) {
            viaEvent = false;
            return;
        }
        LOGGER.info("Carry On：{} 抱起了 {}", carrierId, maidId);
        reportCarried(maidId);
        viaEvent = true;
    }

    /** Carry On 把她放回世界：补一条「放下了」。 */
    public static void putDown(String maidId) {
        if (!carried || maidId == null || !maidId.equals(watchedMaid)) {
            return;
        }
        markDropped(maidId);
    }

    /** 停服、换存档、断线时清干净：下一个世界从头判。 */
    public static void reset() {
        carried = false;
        viaEvent = false;
        watchedMaid = "";
        carriedReports = 0;
        droppedReports = 0;
    }

    private static boolean watching() {
        BridgeServer bridge = LingChatMaidBridge.server();
        return bridge != null
                && bridge.hasClients()
                && !MonitoredMaidHandler.get().isEmpty()
                && MaidBridgeConfig.PERCEPTION_ENABLED.get();
    }

    private static void markDropped(String maidId) {
        carried = false;
        viaEvent = false;
        if (watching()) {
            reportDropped(maidId);
        }
    }

    private static void reportCarried(String maidId) {
        if (carriedReports >= MAX_REPORTS) {
            return;
        }
        carriedReports++;
        emit("maid_carried", maidId);
    }

    private static void reportDropped(String maidId) {
        if (droppedReports >= MAX_REPORTS) {
            return;
        }
        droppedReports++;
        emit("maid_dropped", maidId);
    }

    private static ServerPlayer carrierOf(MinecraftServer server, String maidId) {
        return CarryOnPresent.LOADED ? CarryOnBridge.carrier(server, maidId) : null;
    }

    private static void emit(BridgeServer bridge, String kind, String maidId) {
        JsonObject frame = EventFrames.base(kind, maidId);
        bridge.broadcastEvent(frame);
    }

    /** 只在事件路径里用：桥接对象可能正在关闭。 */
    private static void emit(String kind, String maidId) {
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge != null) {
            emit(bridge, kind, maidId);
        }
    }
}
