package com.lingchat.maidbridge.mixin;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.maid.MaidCondition;
import net.minecraft.world.entity.EntityDimensions;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.Pose;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * 躺着的女仆不要只有 0.2 格高的碰撞箱。
 *
 * <p>原版睡姿的碰撞箱是 0.2 见方，玩家想把她抱起来得把准星压到脚上。睡姿模型以脚下
 * 为原点横着铺开，这里只对女仆换成 1.8 宽、0.8 高，躺着、睡着都能正常点到身体；
 * 被搬运模组抱在手里时仍是站立尺寸，不干扰公主抱的判定。
 */
@Mixin(LivingEntity.class)
public class LivingEntityLyingDimensionsMixin {
    @Inject(method = "getDimensions(Lnet/minecraft/world/entity/Pose;)Lnet/minecraft/world/entity/EntityDimensions;", at = @At("HEAD"), cancellable = true)
    private void lingchat$lyingDimensions(Pose pose, CallbackInfoReturnable<EntityDimensions> cir) {
        if (pose == Pose.SLEEPING && (Object) this instanceof EntityMaid maid && !maid.isPassenger()
                && (MaidCondition.isUnconsciousSynced(maid) || !maid.isSleeping())) {
            cir.setReturnValue(EntityDimensions.scalable(1.8F, 0.8F));
        }
    }
}
