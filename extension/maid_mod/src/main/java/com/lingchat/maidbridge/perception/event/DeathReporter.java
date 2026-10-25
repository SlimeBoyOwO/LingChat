package com.lingchat.maidbridge.perception.event;

import com.github.tartaricacid.touhoulittlemaid.api.event.MaidAndItemTransformEvent;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.MaidBridgeConfig;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.maid.MaidCondition;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.maid.MonitoredMaidHandler;
import com.lingchat.maidbridge.perception.EventFrames;
import com.lingchat.maidbridge.perception.nearby.CreatureMemory;
import com.lingchat.maidbridge.perception.nearby.SurroundingsReader;
import net.minecraft.nbt.CompoundTag;
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
 * <p>被监控的那只女仆自己死了走单独的 {@code maid_death}：这条比「附近死了谁」重要得多，
 * 不吃死亡静默期，也不受距离限制（她就在原地倒下）。她倒下之后所有指令都只会回
 * 「找不到女仆」，插件只能靠这一条知道发生了什么。
 *
 * <p>她主人死了走 {@code owner_death}：同样不吃静默期、不看距离与维度——她可能被留在
 * 家里，但主人复活后她会跟过去（居家模式除外），插件要说的正是这件事。
 */
public final class DeathReporter {
    /** 报了一条之后安静多久（tick）。连着死一片时不至于刷屏。 */
    private static final int COOLDOWN_TICKS = 200;
    /** 驯服类实体的主人 uuid 在 NBT 里的键，与原版 TamableAnimal 一致。 */
    private static final String OWNER_KEY = "Owner";

    private static int cooldown;

    private DeathReporter() {
    }

    public static void onDeath(LivingDeathEvent event) {
        if (!MaidBridgeConfig.PERCEPTION_ENABLED.get()) {
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
        if (maid == null) {
            return;
        }
        Entity attacker = Attackers.actor(event.getSource());
        Entity owner = MaidQueries.owner(maid);
        // 她记得的活物死了：先把死因记进短期记忆，追打落空时才说得出「已经被谁打死了」。
        // 记忆不吃下面那条静默期与距离限制——静默期管的是旁白，不是记性
        if (dead != maid) {
            CreatureMemory.markDead(dead, attacker, maid, dead.level().getGameTime());
        }
        // 她自己死了：比「附近死了谁」重要得多，不吃下面那条静默期。这条也是插件唯一
        // 知道她已经倒下的途径——之后所有指令都只会回「找不到女仆」。
        if (dead == maid) {
            MonitoredMaidHandler.markDead();
            // 致命那一下也是 LivingDamageEvent，已经进了挨打那条的累计。再推出去就是
            // 「先报死、再报伤」，模型会以为自己还活着、还得躲开危险。
            HurtReporter.clear();
            JsonObject frame = EventFrames.base("maid_death", maid.getStringUUID());
            frame.addProperty("attacker", Attackers.name(event.getSource()));
            frame.addProperty("attacker_type", Attackers.typeId(event.getSource()));
            frame.addProperty("damage_type", event.getSource().getMsgId());
            frame.addProperty("attacker_owner", attacker == owner);
            frame.addProperty("attacker_maid", attacker == maid);
            bridge.broadcastEvent(frame);
            return;
        }
        // 主人死了与距离、维度都无关：她可能被留在家里或另一个世界，也一样会跟着
        // 复活的主人过去。旁边死了只生物没法讲这件事，走单独一条。
        if (dead == owner) {
            JsonObject frame = EventFrames.base("owner_death", maid.getStringUUID());
            frame.addProperty("attacker", Attackers.name(event.getSource()));
            frame.addProperty("attacker_type", Attackers.typeId(event.getSource()));
            frame.addProperty("damage_type", event.getSource().getMsgId());
            frame.addProperty("attacker_maid", attacker == maid);
            // 居家模式的她不会跟过去，插件要说的「会跟着回出生点」就不成立了
            frame.addProperty("home_mode", maid.isHomeModeEnable());
            bridge.broadcastEvent(frame);
            return;
        }
        // 她昏迷时听不到附近谁死了；自己的死与主人的死仍然要报
        if (MaidCondition.isUnconscious(maid)) {
            return;
        }
        if (maid.level() != dead.level()) {
            return;
        }
        if (cooldown > 0) {
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
        frame.addProperty("victim_owner", dead == owner);
        frame.addProperty("attacker_owner", attacker == owner);
        frame.addProperty("attacker_maid", attacker == maid);
        bridge.broadcastEvent(frame);
        cooldown = COOLDOWN_TICKS;
    }

    /**
     * 她通过胶片（祭坛 / 神龛）复活了。
     *
     * <p>胶片存的是 {@code saveWithoutId} 的数据，不含 uuid，所以复活出来的是一只**新**
     * 实体：监控目标要跟着改到新的那只，否则聊天与感知会一直对着一个不存在的 uuid。
     * 认的是「胶片里的主人正是我们记住的那位主人」——摆一只别人的女仆不算她复活；
     * 不要求「此刻记着她是死的」，这样游戏重启之后也能认出来。
     */
    public static void onMaidFromItem(MaidAndItemTransformEvent.ToMaid event) {
        if (event.getMaid().level().isClientSide()) {
            return;
        }
        CompoundTag data = event.getData();
        String ownerId = MonitoredMaidHandler.ownerId();
        if (data == null || ownerId.isEmpty() || !data.hasUUID(OWNER_KEY)) {
            return;
        }
        if (!ownerId.equals(data.getUUID(OWNER_KEY).toString())) {
            return;
        }
        EntityMaid maid = event.getMaid();
        MonitoredMaidHandler.markAlive(maid.getStringUUID());
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge == null || !bridge.hasClients()) {
            return;
        }
        JsonObject frame = EventFrames.base("maid_revive", maid.getStringUUID());
        bridge.broadcastEvent(frame);
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
