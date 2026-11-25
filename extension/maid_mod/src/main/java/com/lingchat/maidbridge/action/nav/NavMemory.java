package com.lingchat.maidbridge.action.nav;

import java.util.HashSet;
import java.util.Set;
import net.minecraft.core.BlockPos;

/**
 * 一次目标的记忆：试过的站位、垫不下的位置，以及平铺过几块路。
 *
 * <p>由调用方按自己的粒度持有（建筑按格子），寻路只负责读写。这样换了目标、换了下一次
 * 尝试，之前撞过的死路不会从头再撞一遍。
 */
public final class NavMemory {
    /** 为这个目标试过、但没能站上去的站位。 */
    public final Set<BlockPos> triedStands = new HashSet<>();
    /** 为这个目标试过、但没放成功的垫脚位置。 */
    public final Set<BlockPos> badBlocks = new HashSet<>();
    /** 为这个目标平铺过几块路。 */
    public int bridges;
}
