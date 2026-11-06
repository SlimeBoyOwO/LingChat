package com.lingchat.maidbridge;

import com.lingchat.maidbridge.action.core.MaidActionController;
import com.lingchat.maidbridge.action.core.MaidActionHandler;
import com.lingchat.maidbridge.action.core.MaidSettleHandler;
import com.lingchat.maidbridge.action.attack.MaidAttackController;
import com.lingchat.maidbridge.action.attack.MaidAttackHandler;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageRouter;
import com.lingchat.maidbridge.bridge.Protocol;
import com.lingchat.maidbridge.integration.carry.CarryOnPresent;
import com.lingchat.maidbridge.chat.ChatPushListener;
import com.lingchat.maidbridge.chat.SendChatHandler;
import com.lingchat.maidbridge.perception.item.BackpackWatcher;
import com.lingchat.maidbridge.perception.item.ItemReporter;
import com.lingchat.maidbridge.action.feed.MaidFeeder;
import com.lingchat.maidbridge.action.give.MaidGiveItemHandler;
import com.lingchat.maidbridge.action.headwear.MaidHeadwearHandler;
import com.lingchat.maidbridge.maid.FavoriteFood;
import com.lingchat.maidbridge.maid.MaidStatusHandler;
import com.lingchat.maidbridge.maid.MaidStateHandler;
import com.lingchat.maidbridge.action.task.MaidTaskHandler;
import com.lingchat.maidbridge.maid.MaidCondition;
import com.lingchat.maidbridge.maid.MonitoredMaidHandler;
import com.lingchat.maidbridge.action.mine.MaidMineController;
import com.lingchat.maidbridge.action.mine.MaidMineHandler;
import com.lingchat.maidbridge.action.place.MaidPlaceController;
import com.lingchat.maidbridge.action.place.MaidPlaceHandler;
import com.lingchat.maidbridge.perception.event.AchievementReporter;
import com.lingchat.maidbridge.perception.event.DeathReporter;
import com.lingchat.maidbridge.perception.event.FavoriteFoodReporter;
import com.lingchat.maidbridge.perception.event.HurtReporter;
import com.lingchat.maidbridge.perception.event.ProjectileReporter;
import com.lingchat.maidbridge.perception.nearby.CreatureMemory;
import com.lingchat.maidbridge.perception.PerceptionSampler;
import com.lingchat.maidbridge.perception.query.LookAroundHandler;
import com.lingchat.maidbridge.voice.PlayTtsHandler;
import com.lingchat.maidbridge.voice.TtsPayload;
import net.minecraft.server.MinecraftServer;
import net.neoforged.bus.api.EventPriority;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.fml.ModContainer;
import net.neoforged.fml.common.Mod;
import net.neoforged.fml.config.ModConfig;
import net.neoforged.neoforge.common.NeoForge;
import net.neoforged.neoforge.event.server.ServerStartingEvent;
import net.neoforged.neoforge.event.server.ServerStoppingEvent;
import net.neoforged.neoforge.event.tick.ServerTickEvent;
import net.neoforged.neoforge.network.event.RegisterPayloadHandlersEvent;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * LingChat 与《车万女仆》之间的桥接。
 *
 * <p>三件事：游戏里玩家说的话推给 LingChat、LingChat 里角色的回复变成女仆气泡；世界的
 * 变化（换地形、天黑、下雨、受伤这类）与玩家给她的东西（丢、捡、放进背包、喂食）推给
 * LingChat 记进台词表；以及让角色能指挥女仆走过去、坐下、躺床、看过来。协议与社区那套
 * 桥接模组保持一致，插件端不用为换成我们自己的模组做改动。
 *
 * <p>包按功能分：{@code bridge} 是传输与信封，{@code maid} 是女仆本身，
 * {@code chat} 是文字的两条路，{@code action} 是指挥她动，{@code perception} 是她感知到
 * 什么（{@code perception.nearby} 是她身边那一小片），{@code item} 是她收到与吃掉的东西。
 * 这个类只做装配，不写业务。
 */
@Mod(LingChatMaidBridge.MOD_ID)
public class LingChatMaidBridge {
    public static final String MOD_ID = "lingchat_maid_bridge";
    private static final Logger LOGGER = LoggerFactory.getLogger(MOD_ID);

    private static BridgeServer bridge;
    private static MessageRouter router;

    public LingChatMaidBridge(IEventBus modEventBus, ModContainer modContainer) {
        modContainer.registerConfig(ModConfig.Type.COMMON, MaidBridgeConfig.SPEC);
        modEventBus.addListener(LingChatMaidBridge::onRegisterPayloads);

        // Carry On 是可选依赖：没装时连它的类都不该碰。事件挂在它自己的类里，这里用
        // Class.forName 隔一层——直接引用 CarryOnEvents 的话，它方法签名里的 Carry On
        // 类型可能在类校验阶段就被解析，try/catch 兜不住「类不存在」。
        if (CarryOnPresent.LOADED) {
            try {
                Class.forName("com.lingchat.maidbridge.integration.carry.CarryOnEvents")
                        .getMethod("register")
                        .invoke(null);
            } catch (Throwable brokenCarryOn) {
                LOGGER.warn("Carry On 事件挂不上，改用 NBT 扫描判定被抱起来：{}", brokenCarryOn.toString());
            }
        }

        NeoForge.EVENT_BUS.addListener(LingChatMaidBridge::onServerStarting);
        NeoForge.EVENT_BUS.addListener(LingChatMaidBridge::onServerStopping);
        NeoForge.EVENT_BUS.addListener(LingChatMaidBridge::onServerTick);
        NeoForge.EVENT_BUS.addListener(ChatPushListener::onServerChat);
        NeoForge.EVENT_BUS.addListener(HurtReporter::onDamage);
        // 雪球、鸡蛋这类零伤害的投掷物：不算受伤，但确实砸在了她身上
        NeoForge.EVENT_BUS.addListener(ProjectileReporter::onImpact);
        // 致命伤改成就地昏迷：普通伤害在昏迷期间取消，/kill 与虚空仍走旧死亡
        NeoForge.EVENT_BUS.addListener(EventPriority.LOWEST, MaidCondition::onDamage);
        NeoForge.EVENT_BUS.addListener(EventPriority.HIGHEST, MaidCondition::onIncoming);
        NeoForge.EVENT_BUS.addListener(EventPriority.LOWEST, MaidCondition::onDeath);
        // 附近有东西死了：全世界每次死亡都会发，认位置与静默期在 DeathReporter 里
        NeoForge.EVENT_BUS.addListener(DeathReporter::onDeath);
        // 胶片/神龛把女仆复活：复活出来的是一只新 uuid 的实体，监控目标要跟着换
        NeoForge.EVENT_BUS.addListener(DeathReporter::onMaidFromItem);
        // 主人拿到成就（原版进度）；只认有展示信息的那一类
        NeoForge.EVENT_BUS.addListener(AchievementReporter::onAdvancement);
        // 物品的来去：她捡起来的（连同来源）、喂给她吃的、她自己吃的
        NeoForge.EVENT_BUS.addListener(ItemReporter::onDeath);
        NeoForge.EVENT_BUS.addListener(ItemReporter::onBlockBreak);
        // 掉落一出生就盖来源戳：玩家丢的、方块崩的、生物掉的各归各位，不看主人站得多近
        NeoForge.EVENT_BUS.addListener(ItemReporter::onToss);
        NeoForge.EVENT_BUS.addListener(ItemReporter::onBlockDrops);
        NeoForge.EVENT_BUS.addListener(ItemReporter::onLivingDrops);
        NeoForge.EVENT_BUS.addListener(ItemReporter::onPickupPre);
        NeoForge.EVENT_BUS.addListener(ItemReporter::onPickupPost);
        NeoForge.EVENT_BUS.addListener(ItemReporter::onItemUseFinish);
        // 喂食的优先级有讲究，见 MaidFeeder：普通食物抢在最前面递到她手上并取消，
        // 金苹果只留副本、由车万女仆自己处理，等她真的吃完再补记；两侧都要拦，
        // 否则客户端会把她的坐姿标记在本地翻过去
        NeoForge.EVENT_BUS.addListener(EventPriority.HIGHEST, MaidFeeder::onCapture);
        NeoForge.EVENT_BUS.addListener(MaidFeeder::onAfterEat);
    }

    /** 供推送侧取用；没在监听时为 null。 */
    public static BridgeServer server() {
        return bridge;
    }

    private static void onRegisterPayloads(RegisterPayloadHandlersEvent event) {
        TtsPayload.register(event.registrar("1"));
    }

    private static void onServerStarting(ServerStartingEvent event) {
        if (!MaidBridgeConfig.ENABLED.get()) {
            LOGGER.info("桥接总开关是关的，不监听端口");
            return;
        }
        MinecraftServer server = event.getServer();
        MaidCondition.reset();

        MessageRouter newRouter = new MessageRouter();
        newRouter.register(Protocol.GET_MAID_STATUS, new MaidStatusHandler(server));
        newRouter.register(Protocol.MAID_STATE, new MaidStateHandler(server));
        newRouter.register(Protocol.SET_MONITORED_MAID, new MonitoredMaidHandler(server));
        newRouter.register(Protocol.SEND_CHAT, new SendChatHandler(server));
        newRouter.register(Protocol.MAID_ACTION, new MaidActionHandler(server));
        newRouter.register(Protocol.MAID_LOOK_AROUND, new LookAroundHandler(server));
        newRouter.register(Protocol.MAID_SET_TASK, new MaidTaskHandler(server));
        newRouter.register(Protocol.MAID_ATTACK, new MaidAttackHandler(server));
        newRouter.register(Protocol.MAID_MINE, new MaidMineHandler(server));
        newRouter.register(Protocol.MAID_PLACE, new MaidPlaceHandler(server));
        newRouter.register(Protocol.MAID_GIVE_ITEM, new MaidGiveItemHandler(server));
        newRouter.register(Protocol.MAID_HEADWEAR, new MaidHeadwearHandler(server));
        newRouter.register(Protocol.PLAY_TTS, new PlayTtsHandler(server));
        newRouter.register(Protocol.MAID_SETTLE, new MaidSettleHandler());
        // 她最喜欢吃的东西由插件发过来：这是角色设定，不是模组的全局配置
        newRouter.register(Protocol.SET_FAVORITE_FOOD, request -> {
            FavoriteFood.set(Envelope.strings(Envelope.data(request), "items"));
            return null;
        });
        router = newRouter;

        try {
            bridge = new BridgeServer(MaidBridgeConfig.PORT.get(), newRouter);
            bridge.start();
        } catch (Exception e) {
            // 端口被占之类：不该拖垮游戏，记一笔就行
            LOGGER.error("桥接启动失败：{}", e.getMessage());
            bridge = null;
            router = null;
        }
    }

    private static void onServerStopping(ServerStoppingEvent event) {
        if (bridge != null) {
            try {
                bridge.stop(1000);
            } catch (Exception e) {
                LOGGER.warn("关闭桥接失败：{}", e.getMessage());
            }
            bridge = null;
        }
        router = null;
        // 换存档后旧的女仆 UUID 就失效了，别留着
        MonitoredMaidHandler.reset();
        // 昏迷的服务端索引同理；实体 NBT 里的标记不动，下次加载会重新认回来
        MaidCondition.reset();
        // 感知记着上一个世界的状态，一并清掉
        PerceptionSampler.reset();
        // 附近死亡那条记着静默期，别带到下一个世界
        DeathReporter.clear();
        ProjectileReporter.clear();
        // 她记得的活物同样是一个世界的：换存档后 uuid 都不作数了
        CreatureMemory.clear();
        // 物品的去抖与背包基线同理
        ItemReporter.clear();
        FavoriteFoodReporter.clear();
        FavoriteFood.clear();
        MaidFeeder.clear();
        BackpackWatcher.clear();
        // 动作记着上一个世界的女仆与目标
        MaidActionController.reset();
        // 攻击循环记着上一只女仆与她追的活物
        MaidAttackController.cancel();
        // 挖掘循环记着上一块方块
        MaidMineController.cancel(event.getServer());
        MaidPlaceController.cancel(event.getServer());
    }

    private static void onServerTick(ServerTickEvent.Post event) {
        if (router != null) {
            router.tick();
        }
        // 昏迷的床上计时先走：醒来后这一轮的感知读到的是站姿
        MaidCondition.tick(event.getServer());
        // 感知跟着同一个 tick 走：内部自己判有没有客户端、配置开没开
        PerceptionSampler.tick(event.getServer());
        // 动作也按 tick 推进：每 tick 写一次寻路目标，暂停时自然不计时
        MaidActionController.tick(event.getServer());
        // 追打同样按 tick 推进（与动作互斥，见 MaidAttackController）
        MaidAttackController.tick(event.getServer());
        // 挖掘也按 tick 推进：走过去、朝着方块挖、挖穿了捡东西
        MaidMineController.tick(event.getServer());
        // 放置同理：走到够得着的地方，按原版规则放下一块
        MaidPlaceController.tick(event.getServer());
        // 喂食的待收尾有寿命：没吃成的那口要还给她，金苹果等不到吃完就作废
        MaidFeeder.tick(event.getServer());
        // 主人举着她最爱吃的东西、她凑过来讨食的那一刻
        FavoriteFoodReporter.tick(event.getServer());
    }
}
