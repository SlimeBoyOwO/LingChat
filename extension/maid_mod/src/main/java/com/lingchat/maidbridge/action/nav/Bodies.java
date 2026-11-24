package com.lingchat.maidbridge.action.nav;

import net.minecraft.core.BlockPos;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.level.Level;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;

/** 谁的身体占着这一格：她自己，或者别的活物。 */
public final class Bodies {
    private Bodies() {
    }

    /** 她自己的身体是不是压在这一格里。 */
    public static boolean self(BlockPos pos, Entity self) {
        return self.getBoundingBox().intersects(new AABB(pos));
    }

    /**
     * 她若站到 stand 的格心，身体会不会压住 target。
     *
     * <p>挑站位时她人还没过去，量不了真实碰撞箱，只能按格心估算；等真到了再用
     * {@link #self} 复核。两个都查，才挡得住「站在格边、半个身子探进目标格」这种位置。
     */
    public static boolean selfAt(BlockPos stand, BlockPos target, Entity self) {
        return bodyOverlaps(stand, self.getBbWidth(), self.getBbHeight(), target);
    }

    /** 宽 width、高 height 的身体站在 stand 的格心时，与 target 那一格有没有交叠。 */
    static boolean bodyOverlaps(BlockPos stand, double width, double height, BlockPos target) {
        double half = width / 2.0;
        Vec3 center = Vec3.atBottomCenterOf(stand);
        AABB body = new AABB(
                center.x - half, center.y, center.z - half,
                center.x + half, center.y + height, center.z + half);
        return body.intersects(new AABB(target));
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
