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
import net.minecraft.world.entity.Entity;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.ClipContext;
import net.minecraft.world.level.Level;
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
    /** 射线与落点的距离差在这个范围内算「正好落在贴面上」，不算被挡。 */
    private static final double HIT_EPS = 0.01;

    private Placement() {
    }

    /**
     * 只核几何：给 target 找一个能贴着的面并拼好上下文；找不到给 null。
     *
     * <p>优先用调用方给的面（玩家准星看到的那一面），否则先试脚下，再试她站的那一侧。
     * 点到的支撑块自己可替换时，原版会改成替换支撑块——与 target 对不上，跳过这一面。
     */
    public static BlockPlaceContext context(
            EntityMaid maid, ItemStack stack, BlockPos target,
            BlockPos preferredSupport, Direction preferredFace, Float yaw) {
        for (Direction face : faces(maid, target, preferredFace)) {
            BlockPlaceContext built = build(maid, stack, target, face, yaw);
            if (built != null) {
                return built;
            }
        }
        return null;
    }

    /**
     * 贴得住、而且从她眼睛看得见的第一个面。
     *
     * <p>看不见的面直接跳过：同一块支撑方块从别的面绕进去也算看不见，隔着一堵墙、
     * 一根柱子放到背面是不行的。人挪了位置之后要重新拼，不能拿旧上下文复用。
     */
    public static BlockPlaceContext visibleContext(
            EntityMaid maid, ItemStack stack, BlockPos target,
            BlockPos preferredSupport, Direction preferredFace, Float yaw) {
        for (Direction face : faces(maid, target, preferredFace)) {
            BlockPlaceContext built = build(maid, stack, target, face, yaw);
            if (built != null && visible(maid, built)) {
                return built;
            }
        }
        return null;
    }

    /** 只要指定那一面的几何上下文；那一面贴不住、或者不是贴到 target，给 null。 */
    public static BlockPlaceContext contextForFace(
            EntityMaid maid, ItemStack stack, BlockPos target, Direction face, Float yaw) {
        return build(maid, stack, target, face, yaw);
    }

    /**
     * 从眼睛到落点这一段路上不能先进入任何方块，才算看得见。
     *
     * <p>只比「第一个撞到的方块是不是支撑块」不够：从侧面钻进同一块方块再穿到背面，
     * 撞到的仍是它。改成比距离——射线若在到达落点之前就撞进某个方块，落点就是被挡住的。
     * 栅栏、台阶这类不是整格的支撑也适用，不会因为形状和整格对不上而误判。
     */
    public static boolean visible(EntityMaid maid, BlockPlaceContext context) {
        if (!(maid.level() instanceof ServerLevel level)) {
            return false;
        }
        return unobstructed(
                level, maid.getEyePosition(), context.getClickLocation(), maid);
    }

    /** 从某个站位的眼睛看，target 有没有能贴的面露出来；required 非空时只认那一面。 */
    public static boolean seesFrom(
            EntityMaid maid, BlockPos stand, BlockPos target, Direction required) {
        Vec3 eye = Vec3.atBottomCenterOf(stand).add(0.0, maid.getEyeHeight(), 0.0);
        return seesWith(maid, eye, target, required);
    }

    /** 从她此刻的眼睛看，target 有没有能贴的面露出来；required 非空时只认那一面。 */
    public static boolean seesNow(EntityMaid maid, BlockPos target, Direction required) {
        return seesWith(maid, maid.getEyePosition(), target, required);
    }

    private static boolean seesWith(
            EntityMaid maid, Vec3 eye, BlockPos target, Direction required) {
        if (!(maid.level() instanceof ServerLevel level)) {
            return false;
        }
        if (required != null) {
            return faceVisible(level, eye, target, required, maid);
        }
        for (Direction face : Direction.values()) {
            if (faceVisible(level, eye, target, face, maid)) {
                return true;
            }
        }
        return false;
    }

    /** 贴着 face 的那一面有没有露出来：支撑块在场，眼睛到落点之间没有别的方块。 */
    private static boolean faceVisible(
            Level level, Vec3 eye, BlockPos target, Direction face, Entity source) {
        BlockPos support = target.relative(face.getOpposite());
        if (!level.isLoaded(support)) {
            return false;
        }
        BlockState state = level.getBlockState(support);
        if (state.isAir() || state.getBlock() instanceof LiquidBlock) {
            return false;
        }
        Vec3 point = Vec3.atCenterOf(support).add(
                face.getStepX() * 0.5, face.getStepY() * 0.5, face.getStepZ() * 0.5);
        return unobstructed(level, eye, point, source);
    }

    /** 眼睛到落点这一段路上不能先进入任何方块。 */
    private static boolean unobstructed(Level level, Vec3 eye, Vec3 point, Entity source) {
        double distance = eye.distanceTo(point);
        if (distance < 1.0E-3) {
            return true;
        }
        BlockHitResult hit = level.clip(new ClipContext(
                eye, point, ClipContext.Block.COLLIDER, ClipContext.Fluid.NONE, source));
        if (hit.getType() != HitResult.Type.BLOCK) {
            // 路上一个方块都没撞到：落点自然是露出来的
            return true;
        }
        return hit.getLocation().distanceTo(eye) >= distance - HIT_EPS;
    }

    /** 拼一面的上下文：支撑块、点击点、能否放置、放的是不是 target。 */
    private static BlockPlaceContext build(
            EntityMaid maid, ItemStack stack, BlockPos target, Direction face, Float yaw) {
        if (!(maid.level() instanceof ServerLevel level)) {
            return null;
        }
        BlockPos support = target.relative(face.getOpposite());
        if (!level.isLoaded(support)) {
            return null;
        }
        BlockState supportState = level.getBlockState(support);
        if (supportState.isAir() || supportState.getBlock() instanceof LiquidBlock) {
            return null;
        }
        FakePlayer placer = FakePlayerFactory.getMinecraft(level);
        placer.setPos(maid.getX(), maid.getY(), maid.getZ());
        placer.setYRot(yaw != null ? yaw : maid.getYRot());
        placer.setXRot(maid.getXRot());
        Vec3 point = Vec3.atCenterOf(support).add(
                face.getStepX() * 0.5, face.getStepY() * 0.5, face.getStepZ() * 0.5);
        BlockHitResult hit = new BlockHitResult(point, face, support, false);
        BlockPlaceContext context =
                new BlockPlaceContext(level, placer, InteractionHand.MAIN_HAND, stack, hit);
        if (!context.getClickedPos().equals(target)) {
            return null;
        }
        if (!context.canPlace()) {
            return null;
        }
        return context;
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
