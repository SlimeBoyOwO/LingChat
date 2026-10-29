package com.lingchat.maidbridge.perception.query;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.perception.spatial.LocalTerrain;
import net.minecraft.core.BlockPos;
import net.minecraft.tags.FluidTags;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;

/**
 * 问路时才生成的小图：近处一格一单元，远处两格合并。
 *
 * <p>取她脚下那一层做俯视切片，不懂高差；高差由关系里的 dy 说。图上的字符是
 * {@code @} 她、{@code .} 可走、{@code #} 挡住、{@code ~} 水、{@code !} 岩浆与火、
 * {@code ?} 没加载。图只从传入的地形快照读，不再扫世界。
 */
public record MapProjection(int radius, String text) {
    private static final int NEAR_RADIUS = 8;
    private static final int FAR_CELLS = 6;
    private static final int FAR_CELL_SIZE = 2;

    public static MapProjection render(EntityMaid maid, LocalTerrain terrain) {
        Level level = maid.level();
        BlockPos center = maid.blockPosition();
        int y = center.getY();
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        StringBuilder out = new StringBuilder();

        out.append("近处（一格一单元）：\n");
        near(out, level, terrain, center, y, pos);
        out.append("远处（两格合并）：\n");
        far(out, level, terrain, center, y, pos);
        return new MapProjection(NEAR_RADIUS, out.toString());
    }

    private static void near(
            StringBuilder out, Level level, LocalTerrain terrain, BlockPos center, int y,
            BlockPos.MutableBlockPos pos) {
        for (int dz = -NEAR_RADIUS; dz <= NEAR_RADIUS; dz++) {
            for (int dx = -NEAR_RADIUS; dx <= NEAR_RADIUS; dx++) {
                if (dx != -NEAR_RADIUS) {
                    out.append(' ');
                }
                if (dx == 0 && dz == 0) {
                    out.append('@');
                } else {
                    out.append(cell(level, terrain, center.getX() + dx, y, center.getZ() + dz, pos));
                }
            }
            out.append('\n');
        }
    }

    private static void far(
            StringBuilder out, Level level, LocalTerrain terrain, BlockPos center, int y,
            BlockPos.MutableBlockPos pos) {
        for (int dz = -FAR_CELLS; dz <= FAR_CELLS; dz++) {
            for (int dx = -FAR_CELLS; dx <= FAR_CELLS; dx++) {
                if (dx != -FAR_CELLS) {
                    out.append(' ');
                }
                if (dx == 0 && dz == 0) {
                    out.append('@');
                    continue;
                }
                char merged = '?';
                for (int sx = 0; sx < FAR_CELL_SIZE; sx++) {
                    for (int sz = 0; sz < FAR_CELL_SIZE; sz++) {
                        int x = center.getX() + dx * FAR_CELL_SIZE + sx;
                        int z = center.getZ() + dz * FAR_CELL_SIZE + sz;
                        merged = merge(merged, cell(level, terrain, x, y, z, pos));
                    }
                }
                out.append(merged);
            }
            out.append('\n');
        }
    }

    private static char cell(
            Level level, LocalTerrain terrain, int x, int y, int z, BlockPos.MutableBlockPos pos) {
        BlockState state = terrain.stateAt(x, y, z);
        if (state == null) {
            return '?';
        }
        if (!state.getFluidState().isEmpty()) {
            return state.getFluidState().is(FluidTags.LAVA) ? '!' : '~';
        }
        if (state.is(Blocks.FIRE) || state.is(Blocks.SOUL_FIRE) || state.is(Blocks.MAGMA_BLOCK)) {
            return '!';
        }
        pos.set(x, y, z);
        return state.getCollisionShape(level, pos).isEmpty() ? '.' : '#';
    }

    /** 合并单元取最需要小心的那一种：危险优先，其次挡住，再次没加载，最后才是可走。 */
    private static char merge(char left, char right) {
        if (left == '!' || right == '!') {
            return '!';
        }
        if (left == '~' || right == '~') {
            return '~';
        }
        if (left == '#' || right == '#') {
            return '#';
        }
        if (left == '?' || right == '?') {
            return '?';
        }
        return '.';
    }
}
