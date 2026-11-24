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

    /** 收工：找一处实地站好，并把自己垫的方块拆净。 */
    record Settle() implements NavGoal {
    }
}
