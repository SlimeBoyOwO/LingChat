package com.lingchat.maidbridge.action.nav;

import java.util.Set;
import net.minecraft.core.BlockPos;

/**
 * 调用方交给寻路的边界条件。
 *
 * <p>noBuild 里的格子不许垫脚、不许搭路（建筑传图纸格）；built 里的格子优先当依托；
 * consumeMaterials 决定垫脚方块是从她背包里扣，还是测试时不扣料。
 */
public record NavPolicy(
        Set<BlockPos> noBuild,
        Set<BlockPos> built,
        boolean consumeMaterials) {

    public boolean canBuildOn(BlockPos pos) {
        return !noBuild.contains(pos);
    }
}
