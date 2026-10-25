package com.lingchat.maidbridge.perception.event;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.maid.MaidCondition;
import com.lingchat.maidbridge.maid.MonitoredMaidHandler;
import com.lingchat.maidbridge.perception.EventFrames;
import java.util.Set;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.entity.projectile.ThrowableItemProjectile;
import net.minecraft.world.phys.EntityHitResult;
import net.neoforged.neoforge.event.entity.ProjectileImpactEvent;

/**
 * 雪球、鸡蛋这类砸在身上却不掉血的投掷物。
 *
 * <p>挨打那条（{@link HurtReporter}）只收掉了血的情况：零伤害的投掷物在游戏里根本不算
 * 受伤，砸中了也一直没人说。东西是真的砸在她身上了，所以这里单独报一条。
 *
 * <p>{@code ProjectileImpactEvent} 在命中处理之前发出，认的是射线打中的那只实体，
 * 与伤害判定、无敌帧、护甲都无关。
 *
 * <p>一小段连着砸合成一条：玩家玩开了会一把接一把地扔，逐下发会把台词表刷满。
 * 同一小段里换了投掷物时按最后一件说，谁扔的也取最后一次。
 */
public final class ProjectileReporter {
    /** 会砸在身上但不掉血的投掷物。药水、箭、三叉戟不在这里：它们各有各的效果或已有受伤旁白。 */
    private static final Set<String> HARMLESS = Set.of("minecraft:snowball", "minecraft:egg");
    /** 最后一下之后再等这么久才报，把连着的几下合成一句。 */
    private static final int DEBOUNCE_TICKS = 10;
    /**
     * 两次上报之间的最短间隔。陪她玩的时候一把接一把地扔，每一下都请她开口会没完没了；
     * 攒着每 5 秒报一次，砸中几次照常累计。
     */
    private static final int PUSH_GAP_TICKS = 100;

    private static String maidId = "";
    private static String item = "";
    private static int count;
    private static String thrower = "";
    private static String throwerType = "";
    private static boolean throwerOwner;
    private static int cooldown;
    /** 距离上一次上报还要安静多久。 */
    private static int gap;

    private ProjectileReporter() {
    }

    public static void onImpact(ProjectileImpactEvent event) {
        if (!(event.getProjectile() instanceof ThrowableItemProjectile thrown)) {
            return;
        }
        if (!(event.getRayTraceResult() instanceof EntityHitResult hit)
                || !(hit.getEntity() instanceof EntityMaid maid)) {
            return;
        }
        // 事件在两侧都会发，只有服务端这一份能推给 LingChat
        if (maid.level().isClientSide()) {
            return;
        }
        String monitored = MonitoredMaidHandler.get();
        if (monitored.isEmpty() || !monitored.equals(maid.getStringUUID())) {
            return;
        }
        // 倒下、昏迷时她不该有反应：昏迷期间自发感知本来就是停的
        if (MonitoredMaidHandler.isDead() || !maid.isAlive() || MaidCondition.isUnconscious(maid)) {
            return;
        }
        String thrownItem = BuiltInRegistries.ITEM.getKey(thrown.getItem().getItem()).toString();
        if (!HARMLESS.contains(thrownItem)) {
            return;
        }
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge == null || !bridge.hasClients()) {
            return;
        }
        Entity owner = thrown.getOwner();
        item = thrownItem;
        count++;
        maidId = maid.getStringUUID();
        thrower = owner == null ? "" : owner.getName().getString();
        throwerType = owner == null || owner instanceof Player
                ? ""
                : EntityType.getKey(owner.getType()).toString();
        // 扔她的是不是她主人（也就是 LingChat 里那个玩家）：插件据此说名字而不是念账号名
        throwerOwner = owner != null && owner == maid.getOwner() && maid.getOwner() != null;
        // 每一下都往后推：连着砸的时候等这一波停了再报，而不是每条都报
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

    /** 停服、换存档、以及没人听的时候清干净：别把上一轮的累计算到下一次头上。 */
    public static void clear() {
        maidId = "";
        item = "";
        count = 0;
        thrower = "";
        throwerType = "";
        throwerOwner = false;
        cooldown = 0;
        gap = 0;
    }

    private static void push(BridgeServer bridge) {
        if (count == 0) {
            return;
        }
        JsonObject frame = EventFrames.base("projectile_hit", maidId);
        frame.addProperty("item", item);
        frame.addProperty("count", count);
        frame.addProperty("thrower", thrower);
        frame.addProperty("thrower_type", throwerType);
        frame.addProperty("thrower_owner", throwerOwner);
        bridge.broadcastEvent(frame);
        clear();
        // clear() 会把 gap 归零；这里从这一刻重新计时
        gap = PUSH_GAP_TICKS;
    }
}
