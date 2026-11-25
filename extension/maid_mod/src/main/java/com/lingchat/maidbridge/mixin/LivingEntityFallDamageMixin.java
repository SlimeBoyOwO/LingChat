package com.lingchat.maidbridge.mixin;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.construction.MaidBuildController;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * 施工期间免掉摔落伤害：她为了赶到屋外的落脚点会直接从屋檐跳下去，不该为此掉血。
 *
 * <p>只在施工（含收尾）那一刻生效；平时从高处掉下来照样算伤。
 */
@Mixin(LivingEntity.class)
public abstract class LivingEntityFallDamageMixin {
    @Inject(method = "calculateFallDamage(FF)I", at = @At("HEAD"), cancellable = true)
    private void lingchat$noFallDamageWhileBuilding(
            float fallDistance, float damageMultiplier, CallbackInfoReturnable<Integer> cir) {
        if ((Object) this instanceof EntityMaid maid && MaidBuildController.isRunning(maid)) {
            cir.setReturnValue(0);
        }
    }
}
