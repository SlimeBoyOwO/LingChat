package com.lingchat.maidbridge.action.place;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.ClipContext;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.HitResult;
import net.minecraft.world.phys.Vec3;
import net.neoforged.neoforge.common.util.FakePlayer;
import net.neoforged.neoforge.common.util.FakePlayerFactory;

/**
 * 拼一个「像玩家那样放下」的上下文：贴着的面、要放的那一格、一小份物品。
 *
 * <p>走原版 {@link BlockPlaceContext} 是为了让台阶的上下半、原木朝向、含水这些规则
 * 与玩家亲手放的一致。女仆不是玩家，借一个只用来算朝向的假玩家；背包仍扣她自己的。
 */
public final class Placement {
    private Placement() {
    }

    /**
     * 给 target 找一个能贴着的面并拼好上下文；找不到给 null。
     *
     * <p>优先用调用方给的面（玩家准星看到的那一面），否则先试脚下，再试她站的那一侧。
     * 点到的支撑块自己可替换时，原版会改成替换支撑块——与 target 对不上，跳过这一面。
     */
    public static BlockPlaceContext context(
            EntityMaid maid, ItemStack stack, BlockPos target,
            BlockPos preferredSupport, Direction preferredFace, Float yaw) {
        if (!(maid.level() instanceof ServerLevel level)) {
            return null;
        }
        FakePlayer placer = FakePlayerFactory.getMinecraft(level);
        placer.setPos(maid.getX(), maid.getY(), maid.getZ());
        placer.setYRot(yaw != null ? yaw : maid.getYRot());
        placer.setXRot(maid.getXRot());
        for (Direction face : faces(maid, target, preferredFace)) {
            BlockPos support = target.relative(face.getOpposite());
            if (!level.isLoaded(support)) {
                continue;
            }
            BlockState supportState = level.getBlockState(support);
            if (supportState.isAir() || supportState.getBlock() instanceof LiquidBlock) {
                continue;
            }
            Vec3 point = Vec3.atCenterOf(support).add(
                    face.getStepX() * 0.5, face.getStepY() * 0.5, face.getStepZ() * 0.5);
            BlockHitResult hit = new BlockHitResult(point, face, support, false);
            BlockPlaceContext context =
                    new BlockPlaceContext(level, placer, InteractionHand.MAIN_HAND, stack, hit);
            if (!context.getClickedPos().equals(target)) {
                continue;
            }
            if (!context.canPlace()) {
                continue;
            }
            return context;
        }
        return null;
    }

    /** 从她的眼睛看向放置面：第一命中是这个支撑块，才不是隔着墙放。 */
    public static boolean inSight(EntityMaid maid, BlockPlaceContext context) {
        if (!(maid.level() instanceof ServerLevel level)) {
            return false;
        }
        BlockPos support =
                context.getClickedPos().relative(context.getClickedFace().getOpposite());
        BlockHitResult hit = level.clip(new ClipContext(
                maid.getEyePosition(), context.getClickLocation(),
                ClipContext.Block.COLLIDER, ClipContext.Fluid.NONE, maid));
        return hit.getType() != HitResult.Type.BLOCK || hit.getBlockPos().equals(support);
    }

    /** 试面的顺序：玩家指的那一面、脚下、她站的那一侧、头顶，最后四面都试一遍。 */
    private static List<Direction> faces(EntityMaid maid, BlockPos target, Direction preferred) {
        Set<Direction> faces = new LinkedHashSet<>();
        if (preferred != null) {
            faces.add(preferred);
        }
        faces.add(Direction.UP);
        Direction toward = Direction.getNearest(
                target.getX() - maid.getX(), 0.0, target.getZ() - maid.getZ());
        if (toward.getAxis().isHorizontal()) {
            faces.add(toward);
        }
        faces.add(Direction.DOWN);
        for (Direction side : Direction.Plane.HORIZONTAL) {
            faces.add(side);
        }
        return new ArrayList<>(faces);
    }
}
