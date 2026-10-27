package com.lingchat.maidbridge.mixin;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.core.MaidActionController;
import com.lingchat.maidbridge.maid.MaidCondition;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * 昏迷与桥接按着的躺姿：别让原版与车万女仆把她叫醒。
 *
 * <p>原版每 tick 会检查睡位是不是床，车万女仆的作息清理也会在非休息时段叫醒她；
 * 昏迷不是真的在睡、地上的躺姿也没有床，这两条都得拦掉。躺着的动作被顶掉时
 * 由一个先清掉动作槽、再叫醒她的顺序保证这一下拦不住自己人（见 {@code MaidActionController}）。
 * 真死亡时 {@code LivingDeathEvent} 会先清掉标记，所以尸体会正常醒来并进入死亡姿势。
 */
@Mixin(LivingEntity.class)
public class LivingEntityStopSleepingMixin {
    @Inject(method = "stopSleeping()V", at = @At("HEAD"), cancellable = true)
    private void lingchat$keepLying(CallbackInfo ci) {
        if (!((Object) this instanceof EntityMaid maid)) {
            return;
        }
        // 客户端也要拦：原版 tick 会因睡位不是床而 stopSleeping，睡位一清眼睛就睁开了
        if (MaidCondition.isUnconscious(maid) || MaidCondition.isUnconsciousSynced(maid)
                || MaidActionController.isHoldingLie(maid)) {
            ci.cancel();
        }
    }
}
