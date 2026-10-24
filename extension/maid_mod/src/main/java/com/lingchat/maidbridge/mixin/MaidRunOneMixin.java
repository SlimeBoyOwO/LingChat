package com.lingchat.maidbridge.mixin;

import com.github.tartaricacid.touhoulittlemaid.entity.ai.brain.task.MaidRunOne;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.WanderGuard;
import net.minecraft.server.level.ServerLevel;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * 桥接让她去某处时，别让她同时自己溜达。
 *
 * <p>{@code MaidRunOne} 是车万女仆空闲、工作与休息日程里共用的「四处看看 + 随机走动」
 * 那一个行为（优先级 20）。它随时会往 brain 里写自己的 {@code WALK_TARGET}，和桥接的动作
 * 抢同一份记忆：走到一半被拽去闻花，到了地方又自己走开。动作期间与收尾后的一小段时间里
 * 挡掉它的启动，其余时间照旧（见 {@link WanderGuard}）。
 *
 * <p>只碰服务端行为类，没有客户端代码。
 */
@Mixin(MaidRunOne.class)
public class MaidRunOneMixin {
    @Inject(
            method = "tryStart(Lnet/minecraft/server/level/ServerLevel;"
                    + "Lcom/github/tartaricacid/touhoulittlemaid/entity/passive/EntityMaid;J)Z",
            at = @At("HEAD"),
            cancellable = true)
    private void lingchat$holdRandomWalk(ServerLevel level, EntityMaid maid, long gameTime, CallbackInfoReturnable<Boolean> cir) {
        if (WanderGuard.holds(maid)) {
            cir.setReturnValue(false);
        }
    }
}
