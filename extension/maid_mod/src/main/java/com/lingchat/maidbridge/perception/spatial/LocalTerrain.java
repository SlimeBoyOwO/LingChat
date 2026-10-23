package com.lingchat.maidbridge.perception.spatial;

import net.minecraft.core.BlockPos;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.state.BlockState;

/**
 * 一小片方块的只读快照。
 *
 * <p>「她身边有什么」只有这一处口径：按需把范围里的方块状态读下来，感知与动作共用同一份，
 * 不再各自扫一遍世界。没加载的格子留空，遍历时跳过——与原先逐格判 {@code isLoaded} 等价。
 *
 * <p>遍历顺序是 x、z、y，与原先两处扫描的循环顺序一致：按距离排序遇到等距时，选中哪一块
 * 不会因为这次重构而改变。
 */
public final class LocalTerrain {
    /** 读到的格子。坐标是世界坐标，state 不会是空。 */
    @FunctionalInterface
    public interface CellVisitor {
        void visit(int x, int y, int z, BlockState state);
    }

    private final int minX;
    private final int minY;
    private final int minZ;
    private final int sizeX;
    private final int sizeY;
    private final int sizeZ;
    private final BlockState[] states;

    private LocalTerrain(
            int minX, int minY, int minZ, int sizeX, int sizeY, int sizeZ, BlockState[] states) {
        this.minX = minX;
        this.minY = minY;
        this.minZ = minZ;
        this.sizeX = sizeX;
        this.sizeY = sizeY;
        this.sizeZ = sizeZ;
        this.states = states;
    }

    /**
     * 以 center 为中心读一片。
     *
     * @param radius 水平半径，x、z 各取 center 两侧这么多格
     * @param down   从 center 往下几格
     * @param up     从 center 往上几格
     */
    public static LocalTerrain read(Level level, BlockPos center, int radius, int down, int up) {
        int sizeX = radius * 2 + 1;
        int sizeY = down + up + 1;
        int sizeZ = sizeX;
        int minX = center.getX() - radius;
        int minY = center.getY() - down;
        int minZ = center.getZ() - radius;

        BlockState[] states = new BlockState[sizeX * sizeY * sizeZ];
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        for (int x = 0; x < sizeX; x++) {
            for (int z = 0; z < sizeZ; z++) {
                for (int y = 0; y < sizeY; y++) {
                    pos.set(minX + x, minY + y, minZ + z);
                    if (!level.isLoaded(pos)) {
                        continue;
                    }
                    states[index(x, y, z, sizeY, sizeZ)] = level.getBlockState(pos);
                }
            }
        }
        return new LocalTerrain(minX, minY, minZ, sizeX, sizeY, sizeZ, states);
    }

    /** 按读取顺序遍历读到的格子；没加载的格子不在其中。 */
    public void forEachLoaded(CellVisitor visitor) {
        for (int x = 0; x < sizeX; x++) {
            for (int z = 0; z < sizeZ; z++) {
                for (int y = 0; y < sizeY; y++) {
                    BlockState state = states[index(x, y, z, sizeY, sizeZ)];
                    if (state == null) {
                        continue;
                    }
                    visitor.visit(minX + x, minY + y, minZ + z, state);
                }
            }
        }
    }

    /** 快照里的某一格；超出范围或没加载时给 null。投影只从这一份读，不再碰世界。 */
    public BlockState stateAt(int x, int y, int z) {
        int localX = x - minX;
        int localY = y - minY;
        int localZ = z - minZ;
        if (localX < 0 || localX >= sizeX
                || localY < 0 || localY >= sizeY
                || localZ < 0 || localZ >= sizeZ) {
            return null;
        }
        return states[index(localX, localY, localZ, sizeY, sizeZ)];
    }

    private static int index(int x, int y, int z, int sizeY, int sizeZ) {
        return (x * sizeZ + z) * sizeY + y;
    }
}
