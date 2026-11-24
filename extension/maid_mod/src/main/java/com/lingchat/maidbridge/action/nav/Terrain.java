package com.lingchat.maidbridge.action.nav;

import net.minecraft.core.BlockPos;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.state.BlockState;

/**
 * 寻路自己的地形判定：这一格能不能容她站住。
 *
 * <p>站得住要求脚与头两格为空、脚下有碰撞面。跑酷、挖掘找站位、建筑找落脚点用的是同一条规则，
 * 放在寻路包里，别的模块反过来调用它，而不是寻路去依赖它们。
 */
public final class Terrain {
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
}
