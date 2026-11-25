package com.lingchat.maidbridge.debug;

import com.lingchat.maidbridge.action.construction.MaidBuildController;
import com.lingchat.maidbridge.action.place.PlaceRules;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;

/**
 * 调试指令用的固定图纸：与插件 tools/building.py 里的小木屋、垫脚测试柱同形。
 *
 * <p>这两张只是给「不经过 LLM 直接开一次工」用的，图纸本身不改：小木屋覆盖屋顶外沿、
 * 门廊、烟囱这些要绕到外面去的格子；垫脚柱顶上两层只能先垫一块再走上去。插件那边以后
 * 改图纸不会自动同步到这里，测试时以本文件为准。
 */
final class TestBlueprints {
    /** 小木屋的平面边长；图纸按北面为正面、以中心为原点。 */
    private static final int HOUSE_SIDE = 11;
    /** 找地面时向下看这么多格。 */
    private static final int GROUND_SCAN = 16;
    /** 垫脚柱的边长与高度：2×2、8 格高。 */
    private static final int CLIMB_SIDE = 2;
    private static final int CLIMB_HEIGHT = 8;

    private TestBlueprints() {
    }

    /**
     * 图纸最低一层该落在哪：最低层每一格都往下找脚下方块，取最高的一处地面再往上抬一格。
     *
     * <p>只看最高处，山坡上的图纸不会有一半陷进地里；cells 传 baseY 为 0 的那一份即可。
     */
    static int baseY(Level level, BlockPos anchor, List<MaidBuildController.Target> cells) {
        int minY = Integer.MAX_VALUE;
        for (MaidBuildController.Target cell : cells) {
            minY = Math.min(minY, cell.pos().getY());
        }
        int highest = Integer.MIN_VALUE;
        for (MaidBuildController.Target cell : cells) {
            if (cell.pos().getY() != minY) {
                continue;
            }
            int x = cell.pos().getX();
            int z = cell.pos().getZ();
            for (int y = anchor.getY() + 1; y >= anchor.getY() - GROUND_SCAN; y--) {
                BlockPos pos = new BlockPos(x, y, z);
                if (!level.isLoaded(pos)) {
                    continue;
                }
                BlockState state = level.getBlockState(pos);
                if (state.isAir() || !state.getFluidState().isEmpty()) {
                    continue;
                }
                if (state.getCollisionShape(level, pos).isEmpty()) {
                    continue;
                }
                highest = Math.max(highest, y);
                break;
            }
        }
        return highest == Integer.MIN_VALUE ? anchor.getY() + 1 : highest + 1;
    }

    /** 固定的小木屋：11×11，四层墙、外挑屋顶、门廊与烟囱。 */
    static List<MaidBuildController.Target> house(BlockPos anchor, int baseY) {
        Map<BlockPos, MaidBuildController.Target> plan = new LinkedHashMap<>();
        int half = HOUSE_SIDE / 2;
        int last = HOUSE_SIDE - 1;

        // 地板：外圈石砖，屋里铺云杉木板；门前铺一小块门廊和一级台阶
        for (int z = 0; z < HOUSE_SIDE; z++) {
            for (int x = 0; x < HOUSE_SIDE; x++) {
                boolean edge = x == 0 || x == last || z == 0 || z == last;
                put(plan, anchor, baseY, x, 0, z,
                        edge ? Blocks.STONE_BRICKS : Blocks.SPRUCE_PLANKS, null);
            }
        }
        for (int x = half - 2; x <= half + 2; x++) {
            put(plan, anchor, baseY, x, 0, -1, Blocks.SPRUCE_PLANKS, null);
        }
        for (int x = half - 1; x <= half + 1; x++) {
            put(plan, anchor, baseY, x, 0, -2, Blocks.SPRUCE_STAIRS, facing("south"));
        }

        // 墙：四层，四角与三面中柱是橡木原木；底层石砖，上面云杉木板，窗开两格高
        for (int y = 1; y <= 4; y++) {
            for (int z = 0; z < HOUSE_SIDE; z++) {
                for (int x = 0; x < HOUSE_SIDE; x++) {
                    if (x != 0 && x != last && z != 0 && z != last) {
                        continue;
                    }
                    Block block;
                    Float yaw = null;
                    if ((x == 0 || x == last) && (z == 0 || z == last)) {
                        block = Blocks.OAK_LOG;
                    } else if (x == half && z == 0 && y == 1) {
                        block = Blocks.OAK_DOOR;
                        yaw = facing("north");
                    } else if (x == half && z == 0 && y == 2) {
                        // 门的上半格跟随下半格一起放，不能单独立项
                        continue;
                    } else if (((x == 0 || x == last) && z == half)
                            || (x == half && z == last)) {
                        block = Blocks.OAK_LOG;
                    } else if ((x == half - 1 || x == half + 1) && z == 0 && (y == 1 || y == 2)) {
                        block = Blocks.OAK_LOG;
                    } else if (y == 1) {
                        block = Blocks.STONE_BRICKS;
                    } else if (window(x, z, y)) {
                        block = Blocks.GLASS_PANE;
                    } else {
                        block = Blocks.SPRUCE_PLANKS;
                    }
                    put(plan, anchor, baseY, x, y, z, block, yaw);
                }
            }
        }

        // 屋顶：从墙顶往外挑一格，之后每层向内收一格；外圈台阶，里面铺木板
        int[][] roof = {{5, 6}, {6, 5}, {7, 4}, {8, 3}, {9, 2}, {10, 1}};
        for (int[] layer : roof) {
            int y = layer[0];
            int edge = layer[1];
            for (int z = half - edge; z <= half + edge; z++) {
                for (int x = half - edge; x <= half + edge; x++) {
                    if (Math.abs(x - half) != edge && Math.abs(z - half) != edge) {
                        put(plan, anchor, baseY, x, y, z, Blocks.SPRUCE_PLANKS, null);
                        continue;
                    }
                    Float yaw;
                    if (z == half - edge) {
                        yaw = facing("south");
                    } else if (z == half + edge) {
                        yaw = facing("north");
                    } else if (x == half - edge) {
                        yaw = facing("east");
                    } else {
                        yaw = facing("west");
                    }
                    put(plan, anchor, baseY, x, y, z, Blocks.SPRUCE_STAIRS, yaw);
                }
            }
        }
        put(plan, anchor, baseY, half, 11, half, Blocks.SPRUCE_SLAB, null);
        // 门廊屋檐下留两块实心木板挂灯笼，其余外圈保持台阶
        for (int x : new int[] {half - 1, half + 1}) {
            put(plan, anchor, baseY, x, 5, -1, Blocks.SPRUCE_PLANKS, null);
        }

        // 烟囱：贴在后墙偏东，从地基一路穿出屋顶
        for (int y = 0; y <= 9; y++) {
            put(plan, anchor, baseY, last - 2, y, last, Blocks.STONE_BRICKS, null);
        }
        put(plan, anchor, baseY, last - 2, 10, last, Blocks.STONE_BRICK_SLAB, null);

        // 门廊：两根原木柱撑住屋檐，柱间安栅栏
        for (int x : new int[] {half - 2, half + 2}) {
            for (int y = 1; y <= 4; y++) {
                put(plan, anchor, baseY, x, y, -1, Blocks.OAK_LOG, null);
            }
        }
        for (int x : new int[] {half - 1, half + 1}) {
            put(plan, anchor, baseY, x, 1, -1, Blocks.OAK_FENCE, null);
        }
        // 火把挂两面山墙，灯笼吊在门廊屋檐下
        for (int x : new int[] {1, last - 1}) {
            put(plan, anchor, baseY, x, 3, -1, Blocks.TORCH, null);
            put(plan, anchor, baseY, x, 3, last + 1, Blocks.TORCH, null);
        }
        for (int x : new int[] {half - 1, half + 1}) {
            put(plan, anchor, baseY, x, 4, -1, Blocks.LANTERN, null);
        }
        return new ArrayList<>(plan.values());
    }

    /** 垫脚测试柱：2×2、8 格高，顶上两层只能先垫一块走上去。 */
    static List<MaidBuildController.Target> climb(BlockPos anchor, int baseY) {
        List<MaidBuildController.Target> cells = new ArrayList<>();
        int low = -(CLIMB_SIDE / 2);
        for (int y = 0; y < CLIMB_HEIGHT; y++) {
            for (int x = 0; x < CLIMB_SIDE; x++) {
                for (int z = 0; z < CLIMB_SIDE; z++) {
                    BlockPos pos = new BlockPos(
                            anchor.getX() + low + x, baseY + y, anchor.getZ() + low + z);
                    cells.add(new MaidBuildController.Target(pos, Blocks.COBBLESTONE, null));
                }
            }
        }
        return cells;
    }

    private static void put(
            Map<BlockPos, MaidBuildController.Target> plan, BlockPos anchor, int baseY,
            int x, int y, int z, Block block, Float yaw) {
        int half = HOUSE_SIDE / 2;
        BlockPos pos = new BlockPos(anchor.getX() + x - half, baseY + y, anchor.getZ() + z - half);
        plan.put(pos, new MaidBuildController.Target(pos, block, yaw));
    }

    private static Float facing(String name) {
        return PlaceRules.yawOf(name);
    }

    /** 两格高的窗：正面两对、背面两对、两侧各两对，中柱与门框保持实墙。 */
    private static boolean window(int x, int z, int y) {
        if (y != 2 && y != 3) {
            return false;
        }
        int last = HOUSE_SIDE - 1;
        if (z == 0) {
            return x == 2 || x == 3 || x == 7 || x == 8;
        }
        if (z == last) {
            return x == 3 || x == 4 || x == 6 || x == 7;
        }
        if (x == 0 || x == last) {
            return z == 3 || z == 4 || z == 6 || z == 7;
        }
        return false;
    }
}
