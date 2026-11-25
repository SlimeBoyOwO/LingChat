package com.lingchat.maidbridge.action.nav;

import java.util.function.Predicate;
import net.minecraft.core.BlockPos;

/**
 * 寻路的目标：只描述「到哪里算到」，不描述怎么过去。
 *
 * <p>建筑说「站到能放下这一格的某处」，挖掘说「站到能挖到这一块的某处」，
 * 跟随说「走到人身边」。走、跳下、跑跳、垫脚、搭路全部由寻路自己按环境和预算挑。
 */
public sealed interface NavGoal {
    /** 站到能对 target 动手的位置；canActFrom 由调用方给，判定条件也由它负责。 */
    record Stand(BlockPos target, Predicate<BlockPos> canActFrom) implements NavGoal {
    }

    /**
     * 走到目标附近：玩家身边、某个点；到位按曼哈顿距离判，与动作状态机的口径一致。
     *
     * <p>sameLevel 为真时还必须站到与目标同一层：目标在坎上、脚下在坎下时，光「离得近」
     * 不算到，得先垫上去。
     */
    record Near(BlockPos target, int closeEnough, boolean sameLevel) implements NavGoal {
        /** 这一格算不算到：够近，且（要求时）与目标同层。 */
        public boolean accepts(BlockPos pos) {
            if (sameLevel && pos.getY() != target.getY()) {
                return false;
            }
            return pos.distManhattan(target) <= closeEnough;
        }
    }

    /** 收工：找一处实地站好，并把自己垫的方块拆净。 */
    record Settle() implements NavGoal {
    }
}
