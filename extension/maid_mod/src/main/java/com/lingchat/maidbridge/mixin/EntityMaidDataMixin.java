package com.lingchat.maidbridge.mixin;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.maid.MaidUnconsciousData;
import net.minecraft.network.syncher.EntityDataAccessor;
import net.minecraft.network.syncher.EntityDataSerializers;
import net.minecraft.network.syncher.SynchedEntityData;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * 把昏迷标记加进女仆的同步数据。
 *
 * <p>昏迷状态原本只写在服务端的持久化 NBT 里，客户端读不到；碰撞箱两边都要按同一个
 * 状态算，所以另开一个同步布尔值。字段与车万女仆自己的数据在同一次类初始化里注册，
 * 保证客户端与服务端拿到同一个 id。
 */
@Mixin(EntityMaid.class)
public abstract class EntityMaidDataMixin implements MaidUnconsciousData {
    @Unique
    private static final EntityDataAccessor<Boolean> LINGCHAT_UNCONSCIOUS =
            SynchedEntityData.defineId(EntityMaid.class, EntityDataSerializers.BOOLEAN);

    @Inject(method = "defineSynchedData(Lnet/minecraft/network/syncher/SynchedEntityData$Builder;)V", at = @At("TAIL"))
    private void lingchat$defineUnconscious(SynchedEntityData.Builder builder, CallbackInfo ci) {
        builder.define(LINGCHAT_UNCONSCIOUS, false);
    }

    @Override
    public boolean lingchat$isUnconscious() {
        return ((EntityMaid) (Object) this).getEntityData().get(LINGCHAT_UNCONSCIOUS);
    }

    @Override
    public void lingchat$setUnconscious(boolean value) {
        EntityMaid maid = (EntityMaid) (Object) this;
        if (maid.getEntityData().get(LINGCHAT_UNCONSCIOUS) != value) {
            maid.getEntityData().set(LINGCHAT_UNCONSCIOUS, value);
        }
    }
}
