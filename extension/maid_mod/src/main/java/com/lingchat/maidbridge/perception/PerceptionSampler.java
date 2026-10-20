package com.lingchat.maidbridge.perception;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.MaidBridgeConfig;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.perception.carry.CarryReporter;
import com.lingchat.maidbridge.perception.event.DeathReporter;
import com.lingchat.maidbridge.perception.event.HurtReporter;
import com.lingchat.maidbridge.perception.item.BackpackWatcher;
import com.lingchat.maidbridge.perception.item.ItemReporter;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.maid.MonitoredMaidHandler;
import com.lingchat.maidbridge.perception.nearby.Surroundings;
import com.lingchat.maidbridge.perception.nearby.SurroundingsDiff;
import com.lingchat.maidbridge.perception.nearby.SurroundingsReader;
import com.lingchat.maidbridge.perception.world.PerceptionDiff;
import com.lingchat.maidbridge.perception.world.PerceptionReader;
import com.lingchat.maidbridge.perception.world.PerceptionSnapshot;
import net.minecraft.server.MinecraftServer;

/**
 * 采样的门控与节流：谁在听、多久采一次、这一帧该不该推。
 *
 * <p>它自己不看世界，只按顺序问两层：世界那一层（{@link PerceptionReader} +
 * {@link PerceptionDiff}）与她身边那一层（{@link SurroundingsReader} +
 * {@link SurroundingsDiff}），外加同一采样点上的背包差分（{@link BackpackWatcher}）。
 * 为什么判定都在模组这一侧：LingChat 的插件脚本没有定时器、
 * 也存不下跨调用状态，「持续采样」和「跟上一次比」只能放在常驻、有 tick、记得住上一帧的
 * 这里。这里只推原始字段，中文措辞与「攒够几条再开口」都留给插件。
 *
 * <p>只在「有客户端连着 + 女仆已绑定 + 配置开着」时工作：没人听的时候采样没有意义，
 * 而客户端 0 → N 的那一下会把上一帧丢掉，于是重新连上必然先收到一条
 * {@code state_snapshot}——「未初始化」那一档就靠它。
 */
public final class PerceptionSampler {
    private static PerceptionSnapshot last;
    private static final PerceptionDiff.State state = new PerceptionDiff.State();
    /** 身边这一层的上一帧。它与全局那帧一起采，但基线各自清。 */
    private static Surroundings lastSurroundings;
    private static final SurroundingsDiff.State surroundingsState = new SurroundingsDiff.State();
    private static int sinceSample;
    private static boolean sawClients;

    private PerceptionSampler() {
    }

    /** 每个服务端 tick 调一次（主线程）。 */
    public static void tick(MinecraftServer server) {
        // 被抱起来时她不在世界里，采样什么也看不到；这件事每 tick 单独看一眼
        CarryReporter.tick(server);
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge == null || !bridge.hasClients()) {
            // 没人连着就别攒状态：下次连上必须重新交代「现在是什么样」
            sawClients = false;
            last = null;
            lastSurroundings = null;
            HurtReporter.clear();
            ItemReporter.clear();
            BackpackWatcher.clear();
            return;
        }
        if (!sawClients) {
            sawClients = true;
            last = null;
            lastSurroundings = null;
            state.reset();
            surroundingsState.reset();
        }
        // 挨打那条不受感知开关影响，也不吃采样间隔：它自己被去抖管着
        HurtReporter.tick(bridge);
        // 丢东西、喂饭同样是「一小段合成一条」，只在它自己的去抖到点时推
        ItemReporter.tick(bridge);
        // 附近死亡那条在事件里就推出去了，这里只推进它的静默期
        DeathReporter.tick();
        if (!MaidBridgeConfig.PERCEPTION_ENABLED.get()) {
            BackpackWatcher.clear();
            return;
        }
        EntityMaid maid = monitoredMaid(server);
        if (maid == null) {
            // 区块卸载后她查不到，这段空档要当成没看见：留着上一帧会让下一帧跨着空档做比较，
            // 凭一段过期基线推出「有僵尸出现在你附近」。她回来时全局那层重推一次现状，正好。
            last = null;
            lastSurroundings = null;
            BackpackWatcher.clear();
            return;
        }
        if (++sinceSample < MaidBridgeConfig.PERCEPTION_INTERVAL_TICKS.get()) {
            return;
        }
        sinceSample = 0;

        long tick = server.getTickCount();
        PerceptionSnapshot now = PerceptionReader.read(maid);
        double lowHealthRatio = MaidBridgeConfig.LOW_HEALTH_RATIO.get();
        for (JsonObject frame : PerceptionDiff.events(last, now, state, tick, lowHealthRatio)) {
            bridge.broadcastEvent(frame);
        }
        last = now;

        // 身边这一层跟着同一个采样点走：同一套节流与「谁在听」的门控
        if (MaidBridgeConfig.NOTICE_ENABLED.get()) {
            Surroundings near = SurroundingsReader.read(maid, PerceptionProfiles.auto());
            for (JsonObject frame : SurroundingsDiff.events(lastSurroundings, near, surroundingsState, tick)) {
                bridge.broadcastEvent(frame);
            }
            lastSurroundings = near;
            // 背包也跟着这一轮比：玩家把东西放进她的背包和「身边有变化」是同一件事
            BackpackWatcher.sample(maid, bridge);
        } else {
            // 关着的时候别留着旧基线：重新打开的那一刻要重建基线，而不是拿十分钟前的状态比
            lastSurroundings = null;
            BackpackWatcher.clear();
        }
    }

    /** 停服或换存档时清干净：别把上一个世界的状态拿来比。 */
    public static void reset() {
        last = null;
        lastSurroundings = null;
        state.reset();
        surroundingsState.reset();
        sinceSample = 0;
        sawClients = false;
        HurtReporter.clear();
        ItemReporter.clear();
        BackpackWatcher.clear();
        CarryReporter.reset();
    }

    private static EntityMaid monitoredMaid(MinecraftServer server) {
        EntityMaid maid = MaidQueries.findMaidById(server, MonitoredMaidHandler.get());
        // 她不在已加载区块里时查不到，主人记不下来；见到了就更新一次
        MonitoredMaidHandler.observeOwner(maid);
        return maid;
    }
}
