package com.lingchat.maidbridge.action.nav;

import java.util.HashSet;
import java.util.Set;
import net.minecraft.core.BlockPos;

/**
 * 一次目标的记忆：试过的站位、走不到的站位、垫了几次脚、跳了几次。
 *
 * <p>由调用方按自己的粒度持有（建筑按格子），寻路只负责读写。这样换了目标、换了下一次
 * 尝试，之前撞过的死路不会从头再撞一遍。
 */
public final class NavMemory {
    /** 为这个目标试过、但没能站上去的站位。 */
    public final Set<BlockPos> triedStands = new HashSet<>();
    /** 从她上次站的地方算过、走不到的站位；她没挪窝之前不再算。 */
    public final Set<BlockPos> unreachable = new HashSet<>();
    /** 上一次清走不到的站位时她站的那一格。 */
    public BlockPos searchedFrom;
    /** 跑跳路线没算通时，等到这个游戏刻之后再算。 */
    public long hopUntil;
    /** 为这个目标垫过几次脚（按升高次数算）。 */
    public int steps;
    /** 为这个目标平铺过几块路。 */
    public int bridges;
    /** 为这个目标从高处跳下去过几次。 */
    public int drops;
}
