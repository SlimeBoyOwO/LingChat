package com.lingchat.maidbridge.action.nav;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import net.minecraft.core.BlockPos;
import net.minecraft.tags.FluidTags;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.AABB;

/**
 * 寻路自己的地形判定：能不能站、能不能放垫脚块、伸手够不够得着。
 *
 * <p>跑酷、垫脚、挖掘找站位用的是同一套口径，放在寻路包里，别的模块反过来调用它。
 */
public final class Terrain {
    /** 手够得着的距离，按眼睛到方块外壳算，与原版放置口径一致。 */
    public static final double REACH = 4.5;

    private Terrain() {
    }

    /** 她能站在这一格上吗：自己和头顶是空的，脚下有东西接着。 */
    public static boolean standable(Level level, BlockPos pos) {
        if (!level.isLoaded(pos) || !level.isLoaded(pos.above())) {
            return false;
        }
        BlockState feet = level.getBlockState(pos);
        BlockState head = level.getBlockState(pos.above());
        if (!feet.isAir() && !feet.canBeReplaced()) {
            return false;
        }
        if (!head.isAir() && !head.canBeReplaced()) {
            return false;
        }
        return !level.getBlockState(pos.below()).getCollisionShape(level, pos.below()).isEmpty();
    }

    /** 这一格能不能放东西：空气或可替换，且不是液体。 */
    public static boolean free(Level level, BlockPos pos) {
        BlockState state = level.getBlockState(pos);
        if (!state.isAir() && !state.canBeReplaced()) {
            return false;
        }
        return !(state.getBlock() instanceof LiquidBlock);
    }

    /** 这一格是不是水：水能游过去，但不能站、也不能往上放方块。 */
    public static boolean water(Level level, BlockPos pos) {
        return level.getFluidState(pos).is(FluidTags.WATER);
    }

    /** 这一格是不是实心方块：垫脚块要立在它上面，或者它自己就得先垫出来。 */
    public static boolean solid(Level level, BlockPos pos) {
        BlockState state = level.getBlockState(pos);
        return !state.isAir()
                && state.getFluidState().isEmpty()
                && !state.getCollisionShape(level, pos).isEmpty();
    }

    /** 眼睛够不够得着：按原版口径量眼睛到方块外壳的距离，不是到方块中心。 */
    public static boolean reachable(EntityMaid maid, BlockPos target) {
        return new AABB(target).distanceToSqr(maid.getEyePosition()) <= REACH * REACH;
    }

    /** 站到这一格之后，眼睛够不够得着目标。 */
    public static boolean reachableFrom(EntityMaid maid, BlockPos stand, BlockPos target) {
        double eye = maid.getEyeHeight();
        net.minecraft.world.phys.Vec3 from = net.minecraft.world.phys.Vec3.atBottomCenterOf(stand)
                .add(0.0, eye, 0.0);
        return new AABB(target).distanceToSqr(from) <= REACH * REACH;
    }
}
