package com.lingchat.maidbridge.action.nav;

import net.minecraft.core.BlockPos;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.level.Level;
import net.minecraft.world.phys.AABB;

/** 谁的身体占着这一格：她自己，或者别的活物。 */
public final class Bodies {
    private Bodies() {
    }

    /** 她自己的身体是不是压在这一格里。 */
    public static boolean self(BlockPos pos, Entity self) {
        return self.getBoundingBox().intersects(new AABB(pos));
    }

    /**
     * 这一格里有没有挡着放置的活物。
     *
     * <p>只认活着的生物：掉落物、箭这类实体不挡原版放置，物品也常常堆在工地里不动，
     * 把它们算进来会让一格永远等下去。
     */
    public static boolean other(Level level, BlockPos pos, Entity self) {
        for (Entity entity : level.getEntities(self, new AABB(pos))) {
            if (entity instanceof LivingEntity && entity.isAlive() && !entity.isSpectator()) {
                return true;
            }
        }
        return false;
    }
}
