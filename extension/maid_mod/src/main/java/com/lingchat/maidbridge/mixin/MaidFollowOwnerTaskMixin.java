package com.lingchat.maidbridge.mixin;

import com.github.tartaricacid.touhoulittlemaid.entity.ai.brain.task.MaidFollowOwnerTask;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.LongTaskGuard;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * 她正在执行桥接的长任务时，别让车万女仆的跟随行为把她拽回主人身边。
 *
 * <p>{@code MaidFollowOwnerTask.start} 在主人离她超过「活动半径 + 2」时会直接
 * {@code teleportToOwner}，顺手清掉 {@code WALK_TARGET}——我们刚写进去的挖矿/追击目标
 * 就没了。桥接自己的控制器与这个行为抢的是同一个 brain，没法靠调顺序解决。
 *
 * <p>只在**有任务在跑**的那几十秒里取消这个行为的 start：桥接的攻击、挖方块、走过去/躺床，
 * 以及她自己界面里选的工作模式（非空闲）。攻击类要真有目标才算——目标没了就该恢复默认的
 * 跟随距离，否则她追出去很远之后会一直留在原地。其余时间放行，拾物、找床、找餐桌、跟随全部
 * 保持车万女仆原样——它们读的是同一个 {@code RESTRICT_RADIUS}，靠改半径来放宽会把这些
 * 一起放大，所以这里不改半径。
 *
 * <p>只碰服务端行为类，没有客户端代码。
 */
@Mixin(MaidFollowOwnerTask.class)
public class MaidFollowOwnerTaskMixin {
    @Inject(
            method = "start(Lnet/minecraft/server/level/ServerLevel;"
                    + "Lcom/github/tartaricacid/touhoulittlemaid/entity/passive/EntityMaid;J)V",
            at = @At("HEAD"),
            cancellable = true)
    private void lingchat$keepBridgeTask(ServerLevel level, EntityMaid maid, long gameTime, CallbackInfo ci) {
        if (lingchat$ownerTooFar(maid) && LongTaskGuard.active(maid)) {
            ci.cancel();
        }
    }

    /**
     * 主人是不是已经远到需要处理了。
     *
     * <p>只在这个距离之外取消跟随：主人就在旁边时仍然正常跟随（原版行为），一旦她自己
     * 跑到「原版回头线」之外，就不再把她拽回来或传送回来，让手上的活继续。
     */
    private static boolean lingchat$ownerTooFar(EntityMaid maid) {
        LivingEntity owner = maid.getOwner();
        if (owner == null || owner.level() != maid.level()) {
            return true;
        }
        return maid.distanceTo(owner) > Math.max(3.0F, maid.getRestrictRadius() - 2.0F);
    }
}
