package com.lingchat.maidbridge.perception.nearby;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.world.PerceptionReader;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.decoration.ArmorStand;
import net.minecraft.world.entity.monster.Enemy;
import net.minecraft.world.entity.projectile.ProjectileUtil;
import net.minecraft.world.level.ClipContext;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.EntityHitResult;
import net.minecraft.world.phys.HitResult;
import net.minecraft.world.phys.Vec3;

/**
 * 读一份「她身边」的快照：正前方命中了什么、脚下踩着哪、头顶有没有遮挡、光照多少、
 * 附近有哪些活物、主人离她多远、看不看得见他。
 *
 * <p>位置、视线、光照全部取自她自己——她看不见的东西不该知道。玩家的血量、背包、手持物
 * 一概不读，那是 {@link PerceptionReader} 的活。
 *
 * <p>方位以她的**身体**朝向为准（她不一定面朝哪就看哪），只有视线那条射线用头部朝向。
 */
public final class SurroundingsReader {
    /** 视线射线的最大距离（格）。 */
    private static final double LOOK_DISTANCE = 20.0;

    private SurroundingsReader() {
    }

    /**
     * 按感知档读一份快照。
     *
     * <p>AABB 取两档里较大的半径；候选的「看不看得见」只由遮挡决定，不要求她面朝它。
     * 看不见的活物哪怕在 AABB 里，超过 {@link Surroundings.Profile#heardRadius()} 也会被
     * 筛掉——地表上走着、脚底下洞里几十格外的僵尸不该算她注意到的东西。
     */
    public static Surroundings read(EntityMaid maid, Surroundings.Profile profile) {
        Level level = maid.level();
        BlockPos pos = maid.blockPosition();

        // 脚下：泡在液体里时她踩着的那格是河床，要说的是她泡着的东西
        BlockState inside = level.getBlockState(pos);
        String feetBlock = blockId(inside.getFluidState().isEmpty() ? level.getBlockState(pos.below()) : inside);

        ServerPlayer owner = MaidQueries.owner(maid);
        List<Surroundings.Nearby> raw = new ArrayList<>();
        double radius = Math.max(profile.visibleRadius(), profile.heardRadius());
        AABB box = new AABB(pos).inflate(radius);
        for (LivingEntity other : level.getEntitiesOfClass(
                LivingEntity.class,
                box,
                e -> e != maid && e != owner && e.isAlive() && !e.isSpectator() && !(e instanceof ArmorStand))) {
            double distance = Math.sqrt(maid.distanceToSqr(other));
            if (distance > radius) {
                continue;
            }
            raw.add(new Surroundings.Nearby(
                    typeId(other),
                    categoryOf(other),
                    distance,
                    Surroundings.direction(maid.getYRot(), other.getX() - maid.getX(), other.getZ() - maid.getZ()),
                    maid.hasLineOfSight(other)));
        }
        List<Surroundings.Nearby> eligible = Surroundings.eligible(raw, profile);
        List<Surroundings.Nearby> nearby = Surroundings.trimmed(eligible, profile.limit());

        return new Surroundings(
                maid.getStringUUID(),
                level.dimension().location().toString(),
                gaze(maid, level),
                feetBlock,
                GroundKind.of(feetBlock),
                level.canSeeSky(pos),
                level.dimensionType().hasSkyLight(),
                level.getMaxLocalRawBrightness(pos),
                ownerView(maid, owner),
                nearby,
                eligible.size() > nearby.size());
    }

    /**
     * 以 {@code center} 为中心这一片有哪些能挖的方块，按最近那一块的距离排序、去重、截断。
     *
     * <p>不列空气、液体、没有碰撞体积的东西（草、花、火把）和挖不动的方块。只给注册表 id，
     * 中文名在插件。范围与挖方块那侧的「附近」同口径，这样模型看到的和 {@code maid_mine}
     * 会去找的是同一片。
     */
    public static List<String> blocksNearby(
            EntityMaid maid, BlockPos center, int radius, int vertical, int limit) {
        if (!(maid.level() instanceof ServerLevel level)) {
            return List.of();
        }
        Map<String, Double> nearest = new HashMap<>();
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        for (int dx = -radius; dx <= radius; dx++) {
            for (int dz = -radius; dz <= radius; dz++) {
                for (int dy = -vertical; dy <= vertical; dy++) {
                    pos.set(center.getX() + dx, center.getY() + dy, center.getZ() + dz);
                    if (!level.isLoaded(pos)) {
                        continue;
                    }
                    BlockState state = level.getBlockState(pos);
                    if (state.isAir() || state.getBlock() instanceof LiquidBlock) {
                        continue;
                    }
                    if (state.getCollisionShape(level, pos).isEmpty()) {
                        continue;
                    }
                    if (state.getDestroySpeed(level, pos) < 0.0F) {
                        continue;
                    }
                    nearest.merge(blockId(state), center.distSqr(pos), Math::min);
                }
            }
        }
        return nearest.entrySet().stream()
                .sorted(Map.Entry.comparingByValue())
                .limit(limit)
                .map(Map.Entry::getKey)
                .toList();
    }

    /** 视线正前方第一个命中的东西：方块与实体各打一条射线，取更近的那条。 */
    private static Surroundings.Gaze gaze(EntityMaid maid, Level level) {
        Vec3 eye = maid.getEyePosition();
        Vec3 end = eye.add(maid.getViewVector(1.0F).scale(LOOK_DISTANCE));

        // 方块用 OUTLINE 而不是 COLLIDER：草丛、火把、告示牌这些看得见但没碰撞的也算挡着。
        // 代价是与 hasLineOfSight 的口径不同（玻璃挡得住这条射线，挡不住视线），这是刻意的。
        BlockHitResult blockHit = level.clip(
                new ClipContext(eye, end, ClipContext.Block.OUTLINE, ClipContext.Fluid.NONE, maid));
        double blockDistance =
                blockHit.getType() == HitResult.Type.MISS ? -1.0 : eye.distanceTo(blockHit.getLocation());

        EntityHitResult entityHit = ProjectileUtil.getEntityHitResult(
                maid,
                eye,
                end,
                new AABB(eye, end).inflate(1.0),
                e -> e != maid && e.isAlive() && !e.isSpectator(),
                LOOK_DISTANCE * LOOK_DISTANCE);
        if (entityHit != null) {
            double entityDistance = eye.distanceTo(entityHit.getLocation());
            if (blockDistance < 0 || entityDistance < blockDistance) {
                return new Surroundings.Gaze("entity", typeId(entityHit.getEntity()), entityDistance);
            }
        }
        if (blockDistance < 0) {
            return Surroundings.Gaze.none();
        }
        return new Surroundings.Gaze("block", blockId(level.getBlockState(blockHit.getBlockPos())), blockDistance);
    }

    /** 主人在不在、多远、看不看得见。没有主人、掉线、或在别的维度时如实给「不在」。 */
    private static Surroundings.Owner ownerView(EntityMaid maid, ServerPlayer owner) {
        if (owner == null || owner.hasDisconnected() || owner.level() != maid.level()) {
            return Surroundings.Owner.absent();
        }
        return new Surroundings.Owner(
                owner.getName().getString(),
                Math.sqrt(maid.distanceToSqr(owner)),
                maid.hasLineOfSight(owner),
                true);
    }

    /**
     * 这只生物算哪一类。敌对用 {@link Enemy} 这个标记而不是 {@code MobCategory.MONSTER}：
     * 史莱姆、恶魂、潜影贝这些也敌视玩家，而车万女仆自己不是。
     *
     * <p>公开是因为附近死亡那条（{@code DeathReporter}）要说同一套类别，别在两处各写一份。
     */
    public static String categoryOf(LivingEntity entity) {
        if (entity instanceof ServerPlayer) {
            return "player";
        }
        if (entity instanceof Enemy) {
            return "hostile";
        }
        return entity instanceof EntityMaid ? "maid" : "living";
    }

    private static String blockId(BlockState state) {
        return BuiltInRegistries.BLOCK.getKey(state.getBlock()).toString();
    }

    /** 同上：注册表 id 也只有这一处口径。 */
    public static String typeId(Entity entity) {
        return EntityType.getKey(entity.getType()).toString();
    }
}
