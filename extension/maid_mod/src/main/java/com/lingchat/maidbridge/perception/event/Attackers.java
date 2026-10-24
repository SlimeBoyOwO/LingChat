package com.lingchat.maidbridge.perception.event;

import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.player.Player;

/**
 * 伤害来源 → 「谁动的手」。
 *
 * <p>挨打那条（{@link HurtReporter}）与附近死亡那条（{@link DeathReporter}）都要认这个，
 * 所以放在一处：箭、火球这类投掷物要退到发射者，直接实体才是开枪的那个。
 *
 * <p>三种问法分开：{@link #actor} 用来比「是不是她主人干的」，{@link #typeId} 给生物
 * （中文名由插件按注册表 id 说），{@link #name} 给玩家（账号名跨语言稳定）。别拿
 * {@code name} 去说生物——专用服务端上它只有英文，甚至可能是裸翻译键，中文只在客户端
 * 按玩家语言选得出来。摔、淹、烧这类环境伤害三个都为空，由插件按伤害类型说死因。
 */
public final class Attackers {
    private Attackers() {
    }

    /** 动手的那个实体（没有就是环境伤害）。上报「是不是她主人干的」要用它做比较。 */
    public static Entity actor(DamageSource source) {
        Entity attacker = source.getEntity();
        return attacker != null ? attacker : source.getDirectEntity();
    }

    /** 动手那个东西的名字（玩家名、生物显示名都是它）；环境伤害给空串。 */
    public static String name(DamageSource source) {
        Entity actor = actor(source);
        return actor == null ? "" : actor.getName().getString();
    }

    /** 动手那只生物的注册表 id；玩家与环境伤害给空串。 */
    public static String typeId(DamageSource source) {
        Entity actor = actor(source);
        if (actor == null || actor instanceof Player) {
            return "";
        }
        return EntityType.getKey(actor.getType()).toString();
    }
}
