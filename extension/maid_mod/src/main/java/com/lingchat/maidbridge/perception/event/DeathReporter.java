package com.lingchat.maidbridge.perception.event;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.MaidBridgeConfig;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.maid.MonitoredMaidHandler;
import com.lingchat.maidbridge.perception.EventFrames;
import com.lingchat.maidbridge.perception.nearby.SurroundingsReader;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.neoforged.neoforge.event.entity.living.LivingDeathEvent;

/**
 * 附近有生灵死了：谁死了、被谁弄死的。
 *
 * <p>这是「世界上正在发生什么」里唯一说得清因果的一类——快照差分最多能讲「附近原来有村民，
 * 现在没有了」，说不出为什么（2026-10-16 真机上就是这样：村民被僵尸咬死，她是听玩家打字
 * 才知道的）。所以它走事件，不走采样。
 *
 * <p>{@code LivingDeathEvent} 全世界每次死亡都发，头一件事是认位置：死在感知半径之外的
 * 与她无关，直接丢。
 *
 * <p>同一场屠杀（爆炸、僵尸围村）会连着死好几只，所以**只报第一条**，之后安静
 * {@link #COOLDOWN_TICKS}——玩家要的是知道「附近出事了」，不是一份死亡名单。挨打那条
 * （{@link HurtReporter}）用的是「等这一波停了再合成一条」，这里用不了：死掉的东西不会
 * 复原，合并只会把「谁死了」搅成一锅粥。
 *
 * <p>被监控的那只女仆自己死了**不走这里**：她的死牵着重生、身体回到哪、以及感知基线要
 * 重建，得单独想清楚（现在也不报，是一处已知空缺）。
 */
public final class DeathReporter {
    /** 报了一条之后安静多久（tick）。连着死一片时不至于刷屏。 */
    private static final int COOLDOWN_TICKS = 200;

    private static int cooldown;

    private DeathReporter() {
    }

    public static void onDeath(LivingDeathEvent event) {
        if (cooldown > 0 || !MaidBridgeConfig.PERCEPTION_ENABLED.get()) {
            return;
        }
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge == null || !bridge.hasClients()) {
            return;
        }
        LivingEntity dead = event.getEntity();
        MinecraftServer server = dead.getServer();
        if (server == null) {
            return;
        }
        EntityMaid maid = MaidQueries.findMaidById(server, MonitoredMaidHandler.get());
        if (maid == null || maid == dead || maid.level() != dead.level()) {
            return;
        }
        double distance = Math.sqrt(maid.distanceToSqr(dead));
        if (distance > MaidBridgeConfig.NOTICE_RADIUS.get()) {
            return;
        }

        JsonObject frame = EventFrames.base("creature_death", maid.getStringUUID());
        frame.addProperty("type", SurroundingsReader.typeId(dead));
        frame.addProperty("category", SurroundingsReader.categoryOf(dead));
        frame.addProperty("distance", EventFrames.round(distance));
        frame.addProperty("attacker", Attackers.name(event.getSource()));
        frame.addProperty("attacker_type", Attackers.typeId(event.getSource()));
        frame.addProperty("damage_type", event.getSource().getMsgId());
        // 死的是不是她主人（也就是 LingChat 里那个玩家）／是不是她主人动的手／是不是她自己
        // 动的手：插件据此说「玩家」「你」「另一个玩家」。名字本身不外发，中文说法与身份
        // 判断都留给插件与这三个标记。
        Entity attacker = Attackers.actor(event.getSource());
        Entity owner = MaidQueries.owner(maid);
        frame.addProperty("victim_owner", dead == owner);
        frame.addProperty("attacker_owner", attacker == owner);
        frame.addProperty("attacker_maid", attacker == maid);
        bridge.broadcastEvent(frame);
        cooldown = COOLDOWN_TICKS;
    }

    /** 每个服务端 tick 调一次（主线程）：只用来推进上面那条静默期。 */
    public static void tick() {
        if (cooldown > 0) {
            cooldown--;
        }
    }

    /** 停服、换存档时清干净：别把上个世界的静默期带过来。 */
    public static void clear() {
        cooldown = 0;
    }
}
