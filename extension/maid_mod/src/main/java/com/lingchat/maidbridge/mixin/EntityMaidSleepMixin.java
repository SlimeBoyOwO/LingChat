package com.lingchat.maidbridge.mixin;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.maid.MaidCondition;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * 非床的睡姿不要被车万女仆吸到床中心。
 *
 * <p>{@code onMaidSleep} 每 tick 按睡位把实体摆到床上；昏迷睡在地上时睡位只是脚下
 * 那一格，不跳过就会浮在半空，被抱在手里时还会和搬运抢位置。真躺在床上时不拦，
 * 照常对齐床面。
 */
@Mixin(EntityMaid.class)
public class EntityMaidSleepMixin {
    @Inject(method = "onMaidSleep()V", at = @At("HEAD"), cancellable = true)
    private void lingchat$keepSleepPosition(CallbackInfo ci) {
        EntityMaid maid = (EntityMaid) (Object) this;
        if (MaidCondition.isSleepingOffBed(maid)) {
            ci.cancel();
        }
    }
}
