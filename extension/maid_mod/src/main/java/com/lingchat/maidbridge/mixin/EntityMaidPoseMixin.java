package com.lingchat.maidbridge.mixin;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.core.MaidActionController;
import com.lingchat.maidbridge.maid.MaidCondition;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * 她在地上躺下时，别让车万女仆的游泳管理器把姿势改回站立。
 *
 * <p>{@code EntityMaid.updateSwimming} 每 tick 都跑，不在水里就把姿势重置成
 * {@code STANDING}——躺姿是原版 {@code Pose.SLEEPING}，不挡这一下，下一 tick 就没了。
 * 换姿势还会顺带重算碰撞箱（躺姿只有 0.2 格高），一缩一长会把她一点点往下挪。
 *
 * <p>只在桥接自己挂着的躺下动作里取消；她真在水里游泳、或者已经站起来，都照旧交给
 * 车万女仆。
 */
@Mixin(EntityMaid.class)
public class EntityMaidPoseMixin {
    @Inject(method = "updateSwimming()V", at = @At("HEAD"), cancellable = true)
    private void lingchat$keepLyingPose(CallbackInfo ci) {
        EntityMaid maid = (EntityMaid) (Object) this;
        if (MaidActionController.isGroundLying(maid) || MaidCondition.isUnconscious(maid)) {
            ci.cancel();
        }
    }
}
