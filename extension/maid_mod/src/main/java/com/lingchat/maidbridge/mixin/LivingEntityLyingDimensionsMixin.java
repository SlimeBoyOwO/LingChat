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
 * 为原点横着铺开，这里只对女仆换成 0.9 宽、0.4 高，躺着、睡着都能正常点到身体，
 * 又不会大得挡住旁边的东西；真在床上睡觉仍是原版尺寸（那本来就是床上的睡姿），
 * 被搬运模组抱在手里时也不干扰公主抱的判定。
 */
@Mixin(LivingEntity.class)
public class LivingEntityLyingDimensionsMixin {
    @Inject(method = "getDimensions(Lnet/minecraft/world/entity/Pose;)Lnet/minecraft/world/entity/EntityDimensions;", at = @At("HEAD"), cancellable = true)
    private void lingchat$lyingDimensions(Pose pose, CallbackInfoReturnable<EntityDimensions> cir) {
        if (pose == Pose.SLEEPING && (Object) this instanceof EntityMaid maid && !maid.isPassenger()
                && (MaidCondition.isUnconsciousSynced(maid)
                        || !maid.isSleeping()
                        || !MaidCondition.isSleepingOnBed(maid))) {
            cir.setReturnValue(EntityDimensions.scalable(0.9F, 0.4F));
        }
    }
}
