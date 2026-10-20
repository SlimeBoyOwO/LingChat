package com.lingchat.maidbridge.integration.carry;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.maid.MonitoredMaidHandler;
import com.lingchat.maidbridge.perception.carry.CarryReporter;
import net.neoforged.neoforge.common.NeoForge;
import net.neoforged.neoforge.event.entity.EntityJoinLevelEvent;
import tschipp.carryon.events.EntityPickupEvent;

/**
 * Carry On 自己的事件：抱起来那一刻、放回世界里那一刻。
 *
 * <p>为什么不只靠每 tick 翻玩家的 NBT：Carry On 抱起实体时先把 NBT 存进玩家身上、再把实体
 * 从世界里摘掉，这个过程中间有几帧她既不在世界里、carry 数据也已经写好了；反过来问 NBT
 * 这件事本身又依赖 Carry On 内部的数据形状。它能用，但没必要把它当唯一依据——Carry On
 * 有现成的 {@link EntityPickupEvent}，直接听更稳。
 *
 * <p>本类**只在 Carry On 已加载时才会被注册**：里面引用了它的类，没装时连加载都会失败。
 * 调用方包了 try/catch，挂不上就退回 {@link CarryReporter} 的 NBT 扫描。
 */
public final class CarryOnEvents {
    private CarryOnEvents() {
    }

    public static void register() {
        NeoForge.EVENT_BUS.addListener(CarryOnEvents::onPickup);
        NeoForge.EVENT_BUS.addListener(CarryOnEvents::onJoin);
    }

    /** 被抱起来：事件在她被移出世界之前发，正好拿来判定。 */
    private static void onPickup(EntityPickupEvent event) {
        if (event.target instanceof EntityMaid maid) {
            CarryReporter.pickedUp(maid.getStringUUID(), event.player.getStringUUID());
        }
    }

    /** 被放回世界：她重新加入世界就是「放下了」。 */
    private static void onJoin(EntityJoinLevelEvent event) {
        if (event.getLevel().isClientSide()) {
            return;
        }
        if (event.getEntity() instanceof EntityMaid maid) {
            CarryReporter.putDown(maid.getStringUUID());
        }
    }
}
