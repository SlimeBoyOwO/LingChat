package com.lingchat.maidbridge.mixin;

import com.github.tartaricacid.touhoulittlemaid.entity.ai.brain.task.MaidPickupEntitiesTask;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.WanderGuard;
import net.minecraft.server.level.ServerLevel;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * 桥接让她去某处时，别让她顺手跑去捡地上的东西。
 *
 * <p>捡东西是车万女仆空闲时自己的行为：施工、挖掘或者叫她过来时，她看见掉落物就会
 * 绕过去捡，回来时已经在别的地方了。动作期间与收尾后的一小段时间里挡掉它的启动，
 * 其余时间照旧（见 {@link WanderGuard}）。
 */
@Mixin(MaidPickupEntitiesTask.class)
public class MaidPickupEntitiesTaskMixin {
    @Inject(
            method = "checkExtraStartConditions(Lnet/minecraft/server/level/ServerLevel;"
                    + "Lcom/github/tartaricacid/touhoulittlemaid/entity/passive/EntityMaid;)Z",
            at = @At("HEAD"),
            cancellable = true)
    private void lingchat$holdPickup(
            ServerLevel level, EntityMaid maid, CallbackInfoReturnable<Boolean> cir) {
        if (WanderGuard.holds(maid)) {
            cir.setReturnValue(false);
        }
    }
}
