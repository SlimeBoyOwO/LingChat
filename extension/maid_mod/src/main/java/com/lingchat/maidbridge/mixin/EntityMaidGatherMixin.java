package com.lingchat.maidbridge.mixin;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.perception.item.ItemReporter;
import net.minecraft.core.BlockPos;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * 记下她自己破坏方块的那一刻。
 *
 * <p>车万女仆干活时把掉落直接塞进她的背包（{@code dropResourcesToMaidInv}），不走拾取
 * 事件；背包差分只能看出「多了小麦」。主人在旁边时，原来这份收成会被说成「主人往她的
 * 背包里放了小麦」。这里只记一个时间戳，判定与措辞留给 {@link ItemReporter} 与插件。
 *
 * <p>我们的挖矿也走这个方法，那一路由挖掘收尾自己交代，同样不需要背包差分再报一遍。
 */
@Mixin(EntityMaid.class)
public class EntityMaidGatherMixin {
    @Inject(
            method = "dropResourcesToMaidInv(Lnet/minecraft/world/level/block/state/BlockState;"
                    + "Lnet/minecraft/world/level/Level;Lnet/minecraft/core/BlockPos;"
                    + "Lnet/minecraft/world/level/block/entity/BlockEntity;"
                    + "Lcom/github/tartaricacid/touhoulittlemaid/entity/passive/EntityMaid;"
                    + "Lnet/minecraft/world/item/ItemStack;)V",
            at = @At("HEAD"))
    private void lingchat$markSelfGather(
            BlockState state,
            Level level,
            BlockPos pos,
            BlockEntity blockEntity,
            EntityMaid maid,
            ItemStack tool,
            CallbackInfo ci) {
        ItemReporter.markSelfGather(maid);
    }
}
