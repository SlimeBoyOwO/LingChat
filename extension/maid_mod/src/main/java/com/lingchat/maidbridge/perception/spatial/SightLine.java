package com.lingchat.maidbridge.perception.spatial;

import net.minecraft.core.BlockPos;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.ClipContext;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.HitResult;
import net.minecraft.world.phys.Vec3;

/**
 * 从一只眼睛到一格的视线：先撞上的必须就是那一格，才算看得见。
 *
 * <p>草、花这类没有碰撞体积的方块用轮廓射线，否则它们永远「看不见」。挖矿选落脚点
 * 与台词里的方块清单共用这一处，免得两边对「看得见」各有一套口径。
 */
public final class SightLine {
    private SightLine() {
    }

    public static boolean sees(Level level, Vec3 eye, BlockPos target, Entity source) {
        BlockState state = level.getBlockState(target);
        ClipContext.Block mode = state.getCollisionShape(level, target).isEmpty()
                ? ClipContext.Block.OUTLINE
                : ClipContext.Block.COLLIDER;
        BlockHitResult hit = level.clip(
                new ClipContext(eye, Vec3.atCenterOf(target), mode, ClipContext.Fluid.NONE, source));
        return hit.getType() == HitResult.Type.BLOCK && hit.getBlockPos().equals(target);
    }
}
