package com.lingchat.maidbridge.perception.nearby;

import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.state.BlockState;

/**
 * 头顶这一层空间的三档判定：露天、半遮、屋内。
 *
 * <p>她自己那一格看不看得见天只说明头顶有没有东西；要分清树荫、屋檐和屋子，还得看这一小片
 * 里有没有露天的地方。于是以她脚下那一格为中心取半径四格的圆盘：从她那一格横着连过去，
 * 中间隔着挡路方块的那一列不算（墙、关着的门都算隔着），剩下的列里一块露天的都没有才算屋内。
 *
 * <p>只看她脚下这一层，不算上下坡；开着的门会把门外那片露天算进来，屋里于是也会落到半遮。
 * 光凭方块也分不清石屋与山洞，材质只回 id，说法留给插件。
 */
public final class OverheadScan {
    /** 往四周看几格：四格够覆盖一间小屋，也够在屋檐下摸到边上的天。 */
    public static final int SCAN_RADIUS = 4;
    /** 往上找头顶方块最多找几格，再高只看档位。 */
    public static final int MAX_COVER_HEIGHT = 16;
    /** 头顶那块方块离她头顶三格以内算压得低。 */
    public static final int LOW_BAND_GAP = 3;

    public static final String OPEN = "open";
    public static final String PARTIAL = "partial";
    public static final String ENCLOSED = "enclosed";
    public static final String BAND_LOW = "low";
    public static final String BAND_HIGH = "high";

    private OverheadScan() {
    }

    /** 没有天空的维度整项留空；她自己那一格就露天时不再往下扫。 */
    public static Surroundings.Overhead read(Level level, BlockPos feet) {
        if (!level.dimensionType().hasSkyLight()) {
            return Surroundings.Overhead.none();
        }
        if (level.canSeeSky(feet)) {
            return new Surroundings.Overhead(OPEN, "", "");
        }
        int open = 0;
        for (int dx = -SCAN_RADIUS; dx <= SCAN_RADIUS; dx++) {
            for (int dz = -SCAN_RADIUS; dz <= SCAN_RADIUS; dz++) {
                if (dx * dx + dz * dz > SCAN_RADIUS * SCAN_RADIUS) {
                    continue;
                }
                BlockPos at = feet.offset(dx, 0, dz);
                if (!level.isLoaded(at) || wallBetween(level, feet, dx, dz)) {
                    continue;
                }
                if (level.canSeeSky(at)) {
                    open++;
                }
            }
        }
        BlockPos cover = coverAbove(level, feet);
        String band = cover == null ? BAND_HIGH : coverBand(cover.getY() - feet.getY() - 2);
        return new Surroundings.Overhead(
                classify(open), cover == null ? "" : blockId(level, cover), band);
    }

    /** 半遮还是屋内：这一圈里还有露天的一列就是半遮（树荫、屋檐、洞口）。 */
    public static String classify(int openColumns) {
        return openColumns > 0 ? PARTIAL : ENCLOSED;
    }

    /** 头顶方块离她头顶还有几格空气：三格以内算压得低，再高只说高。 */
    public static String coverBand(int gap) {
        return gap <= LOW_BAND_GAP ? BAND_LOW : BAND_HIGH;
    }

    /** 她脚下那一格到候选格之间有没有挡路的方块；两端不算。 */
    private static boolean wallBetween(Level level, BlockPos feet, int dx, int dz) {
        int steps = Math.max(Math.abs(dx), Math.abs(dz));
        for (int step = 1; step < steps; step++) {
            int x = feet.getX() + Math.round(dx * (float) step / steps);
            int z = feet.getZ() + Math.round(dz * (float) step / steps);
            if (solid(level, new BlockPos(x, feet.getY(), z))) {
                return true;
            }
        }
        return false;
    }

    private static boolean solid(Level level, BlockPos pos) {
        return level.isLoaded(pos) && level.getBlockState(pos).blocksMotion();
    }

    /** 头顶第一块挡路的方块；十六格内没有给 null。 */
    private static BlockPos coverAbove(Level level, BlockPos feet) {
        for (int dy = 2; dy <= MAX_COVER_HEIGHT; dy++) {
            BlockPos at = feet.above(dy);
            if (!level.isLoaded(at)) {
                return null;
            }
            BlockState state = level.getBlockState(at);
            if (state.blocksMotion()) {
                return at;
            }
        }
        return null;
    }

    private static String blockId(Level level, BlockPos pos) {
        return BuiltInRegistries.BLOCK.getKey(level.getBlockState(pos).getBlock()).toString();
    }
}
