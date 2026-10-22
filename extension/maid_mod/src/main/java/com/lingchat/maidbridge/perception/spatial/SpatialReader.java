package com.lingchat.maidbridge.perception.spatial;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.perception.nearby.Surroundings;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.level.ClipContext;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.HitResult;
import net.minecraft.world.phys.Vec3;
import net.minecraft.world.phys.shapes.VoxelShape;

/**
 * 她眼里的危险方块：岩浆、火、岩浆块、细雪。
 *
 * <p>与台词里的方块清单同一口径：只算露在外面、视线真的落到的那几格——埋在地下的岩浆
 * 不告密，隔着一堵墙的也不算她看见。她脚下那一列交给 {@code ground_change}，这里只报
 * 「边上」的，同一个地方不会一次说两句。
 *
 * <p>每种方块只留最近的一格：模型要知道的是「附近有岩浆」，不是一张岩浆坐标表。
 */
public final class SpatialReader {
    /** 看多远：走过岩浆边就够；再远报起来像全知。 */
    private static final int RADIUS = 8;
    private static final int DOWN = 4;
    private static final int UP = 2;
    private static final Set<Block> HAZARDS = Set.of(
            Blocks.LAVA, Blocks.FIRE, Blocks.SOUL_FIRE, Blocks.MAGMA_BLOCK, Blocks.POWDER_SNOW);

    private SpatialReader() {
    }

    public static SpatialView read(EntityMaid maid) {
        Level level = maid.level();
        BlockPos feet = maid.blockPosition();
        Vec3 eye = maid.getEyePosition();
        LocalTerrain terrain = LocalTerrain.read(level, feet, RADIUS, DOWN, UP);
        Map<String, SpatialView.Hazard> nearest = new LinkedHashMap<>();
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        terrain.forEachLoaded((x, y, z, state) -> {
            Block block = state.getBlock();
            if (!HAZARDS.contains(block)) {
                return;
            }
            // 同一列（她身上、脚下踩着的那块）走 ground_change，这里不重复
            if (x == feet.getX() && z == feet.getZ()) {
                return;
            }
            pos.set(x, y, z);
            if (!exposed(level, pos) || !sees(level, eye, pos, state, maid)) {
                return;
            }
            double distance = Math.sqrt(feet.distSqr(pos));
            String id = BuiltInRegistries.BLOCK.getKey(block).toString();
            SpatialView.Hazard old = nearest.get(id);
            if (old != null && old.distance() <= distance) {
                return;
            }
            String direction = Surroundings.direction(
                    maid.getYRot(), x - maid.getX(), z - maid.getZ());
            nearest.put(id, new SpatialView.Hazard(id, distance, direction));
        });
        return new SpatialView(maid.getStringUUID(), new ArrayList<>(nearest.values()));
    }

    /**
     * 视线真的落到这一格了吗：射线打向它轮廓形状的中心，先撞上的必须就是它。
     *
     * <p>不直接用 {@link SightLine}：火的轮廓只有贴地薄薄一层，打向方块中心永远落空，
     * 火就永远「看不见」。按形状包围盒取一个一定在形状里的点，岩浆、火、细雪都通用。
     */
    private static boolean sees(
            Level level, Vec3 eye, BlockPos pos, BlockState state, EntityMaid maid) {
        VoxelShape shape = state.getCollisionShape(level, pos);
        if (shape.isEmpty()) {
            shape = state.getShape(level, pos);
        }
        if (shape.isEmpty()) {
            return false;
        }
        AABB box = shape.bounds().move(pos);
        Vec3 point = new Vec3(
                (box.minX + box.maxX) / 2.0, (box.minY + box.maxY) / 2.0,
                (box.minZ + box.maxZ) / 2.0);
        BlockHitResult hit = level.clip(new ClipContext(
                eye, point, ClipContext.Block.OUTLINE, ClipContext.Fluid.NONE, maid));
        return hit.getType() == HitResult.Type.BLOCK && hit.getBlockPos().equals(pos);
    }

    /** 六面里有一面不是实心遮挡就算露出来了；与身边那一层的判法一致。 */
    private static boolean exposed(Level level, BlockPos pos) {
        for (Direction direction : Direction.values()) {
            BlockPos side = pos.relative(direction);
            if (level.isLoaded(side) && !level.getBlockState(side).canOcclude()) {
                return true;
            }
        }
        return false;
    }
}
