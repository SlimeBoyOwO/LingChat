package com.lingchat.maidbridge.mixin;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.maid.FavoriteFood;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.item.crafting.Ingredient;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * 把「最喜欢的食物」接进车万女仆自己的讨食任务。
 *
 * <p>她的讨食任务只认一件诱惑物（配置里的那一件，默认蛋糕）。主人手里拿着的若是
 * 配置里填的最喜欢食物，这里就把那件东西当成诱惑物交出去，于是讨食的动画、走到跟前、
 * 松手后的收尾全走原版那条路，不用另写一份。原版那件仍照常有效。
 */
@Mixin(EntityMaid.class)
public abstract class EntityMaidTemptationMixin {
    @Inject(
            method = "getTemptationItem()Lnet/minecraft/world/item/crafting/Ingredient;",
            at = @At("HEAD"),
            cancellable = true)
    private void lingchat$favoriteFood(CallbackInfoReturnable<Ingredient> cir) {
        EntityMaid maid = (EntityMaid) (Object) this;
        LivingEntity owner = maid.getOwner();
        if (owner != null && FavoriteFood.holding(owner.getMainHandItem())) {
            cir.setReturnValue(Ingredient.of(owner.getMainHandItem().getItem()));
        }
    }
}
