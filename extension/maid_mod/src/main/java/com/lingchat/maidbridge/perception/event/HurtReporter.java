package com.lingchat.maidbridge.perception.event;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.maid.MonitoredMaidHandler;
import com.lingchat.maidbridge.perception.EventFrames;
import net.neoforged.neoforge.event.entity.living.LivingDamageEvent;

/**
 * 女仆挨打的上报：一小段连续挨打合成一条。
 *
 * <p>刷怪塔里一秒能挨好几下，逐下推会把台词表刷满，所以这里只累计、去抖，等这一波停了
 * 再推一条。
 *
 * <p>这条只能由模组推：被打的是谁、被谁打的、掉了几点血、现在还剩多少，只有游戏这一侧
 * 知道。{@code LivingDamageEvent} 全世界每次受伤都会发，所以头一件事是按 uuid 认人 ——
 * 不是被监控的那只就当场丢。
 */
public final class HurtReporter {
    /** 一小段连续挨打合成一条：最后一下之后再等这么久才报。 */
    private static final int DEBOUNCE_TICKS = 10;
    /**
     * 两次上报之间的最短间隔。战斗中每挨一下都会重置去抖，光靠上面那个会变成
     * 「每十来个 tick 报一条」，插件那边就要跟着不停请角色开口。攒着每 5 秒报一次，
     * 伤害与次数照常累计。
     */
    private static final int PUSH_GAP_TICKS = 100;

    private static double damage;
    private static int count;
    private static String maidId = "";
    private static String attacker = "";
    private static String damageType = "";
    private static String attackerType = "";
    private static boolean attackerOwner;
    private static double health;
    private static double maxHealth;
    private static int cooldown;
    /** 距离上一次上报还要安静多久。 */
    private static int gap;

    private HurtReporter() {
    }

    public static void onDamage(LivingDamageEvent.Post event) {
        if (!(event.getEntity() instanceof EntityMaid maid)) {
            return;
        }
        String monitored = MonitoredMaidHandler.get();
        if (monitored.isEmpty() || !monitored.equals(maid.getStringUUID())) {
            return;
        }
        // 她已经倒下：致命那一下也是 LivingDamageEvent，与死亡事件的先后不保证。
        // 死亡分支会清一次累计，这里再挡一下，免得倒下的那一击又在十 tick 后补报出来。
        if (MonitoredMaidHandler.isDead() || !maid.isAlive()) {
            return;
        }
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge == null || !bridge.hasClients()) {
            return;
        }
        float damageTaken = event.getNewDamage();
        if (damageTaken <= 0) {
            return;
        }
        damage += damageTaken;
        count++;
        maidId = maid.getStringUUID();
        attacker = Attackers.name(event.getSource());
        attackerType = Attackers.typeId(event.getSource());
        // 打她的是不是她主人（也就是 LingChat 里那个玩家）：插件据此说「玩家」而不是念账号名
        attackerOwner = Attackers.actor(event.getSource()) == maid.getOwner() && maid.getOwner() != null;
        damageType = event.getSource().getMsgId();
        health = maid.getHealth();
        maxHealth = maid.getMaxHealth();
        // 每挨一下都往后推：连着挨打时等这一波停了再报，而不是每条都报
        cooldown = DEBOUNCE_TICKS;
    }

    /** 每个服务端 tick 调一次（主线程）：这一波停了、且离上次上报够久了，再合成一条推出去。 */
    public static void tick(BridgeServer bridge) {
        if (gap > 0) {
            gap--;
        }
        if (cooldown > 0) {
            cooldown--;
        }
        if (cooldown == 0 && gap == 0) {
            push(bridge);
        }
    }

    /** 停服、换存档、以及没人听的时候清干净：别把上一波伤算到下一次头上。 */
    public static void clear() {
        damage = 0;
        count = 0;
        maidId = "";
        attacker = "";
        attackerType = "";
        attackerOwner = false;
        damageType = "";
        cooldown = 0;
        gap = 0;
    }

    private static void push(BridgeServer bridge) {
        if (count == 0) {
            return;
        }
        JsonObject frame = EventFrames.base("maid_hurt", maidId);
        frame.addProperty("damage", damage);
        frame.addProperty("count", count);
        frame.addProperty("attacker", attacker);
        frame.addProperty("attacker_type", attackerType);
        frame.addProperty("attacker_owner", attackerOwner);
        frame.addProperty("damage_type", damageType);
        frame.addProperty("health", health);
        frame.addProperty("max_health", maxHealth);
        bridge.broadcastEvent(frame);
        clear();
        // clear() 会把 gap 归零；这里从这一刻重新计时
        gap = PUSH_GAP_TICKS;
    }
}
