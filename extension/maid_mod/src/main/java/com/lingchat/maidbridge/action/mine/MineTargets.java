package com.lingchat.maidbridge.action.mine;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.TagKey;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.ClipContext;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.HitResult;
import net.minecraft.world.phys.Vec3;
import net.neoforged.neoforge.items.wrapper.CombinedInvWrapper;

/**
 * 挖方块时碰游戏的那一层：玩家看着哪块、她周围有哪些同类方块、站哪能挖到。
 *
 * <p>判定与算式在 {@link MineRules}，这里只把 Minecraft 的事实读出来。找落脚点的思路
 * 来自车万女仆的挖矿附属模组（MaidTask）：在目标周围找一块站得下、够得着、看得见、
 * 而且寻路能走到的位置，而不是直接往方块里面寻路。
 */
public final class MineTargets {
    /** 她站着时眼睛到方块中心能到多远：比原版玩家略宽一点，免得站在方块正下方却够不着。 */
    private static final double EYE_REACH = MineRules.REACH + 0.5F;
    /** 找落脚点时最多验几个候选的寻路：每个候选都要算一次路径，不能把一圈全试完。 */
    private static final int PATH_CHECKS = 6;

    private MineTargets() {
    }

    public static String blockId(BlockState state) {
        return BuiltInRegistries.BLOCK.getKey(state.getBlock()).toString();
    }

    /**
     * 目标写法展开成实际要比对的方块集合。
     *
     * <p>{@code #namespace:path} 是方块标签，插件把「木头」这类泛称翻成
     * {@code #minecraft:logs}；普通 id 只认这一块，矿石再带上深层变体（地下几乎只剩深层矿）。
     */
    public static Set<Block> targets(String blockId) {
        Set<Block> blocks = new HashSet<>();
        if (blockId.startsWith("#")) {
            ResourceLocation tagId = ResourceLocation.tryParse(blockId.substring(1));
            if (tagId == null) {
                return blocks;
            }
            TagKey<Block> tag = TagKey.create(Registries.BLOCK, tagId);
            for (Block block : BuiltInRegistries.BLOCK) {
                if (block != Blocks.AIR && block.defaultBlockState().is(tag)) {
                    blocks.add(block);
                }
            }
            return blocks;
        }
        ResourceLocation id = ResourceLocation.tryParse(blockId);
        Block block = id == null ? null : BuiltInRegistries.BLOCK.get(id);
        if (block == null || block == Blocks.AIR) {
            return blocks;
        }
        blocks.add(block);
        if (id.getPath().endsWith("_ore") && !id.getPath().startsWith("deepslate_")) {
            ResourceLocation deep = ResourceLocation.tryParse(id.getNamespace() + ":deepslate_" + id.getPath());
            Block deepBlock = deep == null ? null : BuiltInRegistries.BLOCK.get(deep);
            if (deepBlock != null && deepBlock != Blocks.AIR) {
                blocks.add(deepBlock);
            }
        }
        return blocks;
    }

    /**
     * 以 {@code center} 为中心找这一种方块，按离她远近排好。
     *
     * <p>只做粗筛（是不是这一族方块、能不能挖、在不在活动范围内），落脚点留给真正轮到的
     * 那几块再验：一圈几千个方块里逐个算路径是算不起的。
     */
    public static List<BlockPos> nearest(
            EntityMaid maid, BlockPos center, String blockId, int radius, int limit) {
        Set<Block> targets = targets(blockId);
        List<BlockPos> found = new ArrayList<>();
        if (targets.isEmpty() || !(maid.level() instanceof ServerLevel level)) {
            return found;
        }
        int vertical = Math.min(radius, 8);
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        for (int dx = -radius; dx <= radius; dx++) {
            for (int dz = -radius; dz <= radius; dz++) {
                for (int dy = -vertical; dy <= vertical; dy++) {
                    pos.set(center.getX() + dx, center.getY() + dy, center.getZ() + dz);
                    if (!level.isLoaded(pos)) {
                        continue;
                    }
                    BlockState state = level.getBlockState(pos);
                    if (state.isAir() || !targets.contains(state.getBlock())) {
                        continue;
                    }
                    // 水与岩浆不是「挖」出来的：让她们去舀是另一回事
                    if (state.getBlock() instanceof LiquidBlock) {
                        continue;
                    }
                    if (!MineRules.mineable(state.getDestroySpeed(level, pos))) {
                        continue;
                    }
                    if (maid.hasRestriction() && !maid.isWithinRestriction(pos)) {
                        continue;
                    }
                    found.add(pos.immutable());
                }
            }
        }
        found.sort(Comparator.comparingDouble(block -> maid.distanceToSqr(Vec3.atCenterOf(block))));
        return found.size() > limit ? new ArrayList<>(found.subList(0, limit)) : found;
    }

    /**
     * 站哪能挖到这一块：站得下、够得着、看得见，而且寻路能走到。找不到给 null。
     *
     * <p>看得见按准星射线算：视线先碰到的必须是目标方块本身，隔着一堵墙挖是不行的。
     * 草、花这类没有碰撞体积的方块用轮廓射线，否则它们永远「看不见」。
     */
    public static BlockPos standFor(EntityMaid maid, BlockPos target) {
        if (!(maid.level() instanceof ServerLevel level)) {
            return null;
        }
        List<BlockPos> candidates = new ArrayList<>();
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        for (int dx = -2; dx <= 2; dx++) {
            for (int dy = -2; dy <= 2; dy++) {
                for (int dz = -2; dz <= 2; dz++) {
                    if (dx == 0 && dy == 0 && dz == 0) {
                        continue;
                    }
                    pos.setWithOffset(target, dx, dy, dz);
                    BlockPos stand = pos.immutable();
                    if (!standable(level, stand)) {
                        continue;
                    }
                    if (maid.hasRestriction() && !maid.isWithinRestriction(stand)) {
                        continue;
                    }
                    if (!reachableFrom(level, stand, target, maid)) {
                        continue;
                    }
                    candidates.add(stand);
                }
            }
        }
        candidates.sort(Comparator.comparingDouble(stand -> maid.distanceToSqr(Vec3.atBottomCenterOf(stand))));
        int checks = Math.min(candidates.size(), PATH_CHECKS);
        for (int i = 0; i < checks; i++) {
            BlockPos stand = candidates.get(i);
            // 已经站在跟前就不用再问寻路了
            if (stand.closerToCenterThan(maid.position(), 2.0) || maid.canPathReach(stand)) {
                return stand;
            }
        }
        return null;
    }

    /** 她此刻能不能直接挖到这一块：够得着、看得见。 */
    public static boolean canMine(EntityMaid maid, BlockPos target) {
        if (!(maid.level() instanceof ServerLevel level)) {
            return false;
        }
        Vec3 eye = maid.getEyePosition();
        if (eye.distanceTo(Vec3.atCenterOf(target)) > EYE_REACH) {
            return false;
        }
        return inSight(level, eye, target, maid);
    }

    /** 手上或背包里有没有能挖这一块、并且不掉落一空的工具。 */
    public static boolean hasCorrectTool(EntityMaid maid, BlockState state) {
        if (!state.requiresCorrectToolForDrops()) {
            return true;
        }
        if (maid.getMainHandItem().isCorrectToolForDrops(state)) {
            return true;
        }
        CombinedInvWrapper backpack = maid.getAvailableBackpackInv();
        for (int slot = 0; slot < backpack.getSlots(); slot++) {
            if (backpack.getStackInSlot(slot).isCorrectToolForDrops(state)) {
                return true;
            }
        }
        return false;
    }

    /**
     * 把背包里挖这一块最快的工具换到手上；手上本来就是最好的就不动。
     *
     * <p>只比较「正确工具」之间的速度：剑挖树叶再快也不该拿去挖石头，它掉不了东西。
     */
    public static void equipBestTool(EntityMaid maid, BlockState state) {
        ItemStack hand = maid.getMainHandItem();
        boolean handCorrect = hand.isCorrectToolForDrops(state);
        float bestSpeed = handCorrect ? hand.getDestroySpeed(state) : -1.0F;
        CombinedInvWrapper backpack = maid.getAvailableBackpackInv();
        int bestSlot = -1;
        for (int slot = 0; slot < backpack.getSlots(); slot++) {
            ItemStack stack = backpack.getStackInSlot(slot);
            if (stack.isEmpty() || !stack.isCorrectToolForDrops(state)) {
                continue;
            }
            float speed = stack.getDestroySpeed(state);
            if (speed > bestSpeed) {
                bestSpeed = speed;
                bestSlot = slot;
            }
        }
        if (bestSlot < 0) {
            return;
        }
        ItemStack tool = backpack.extractItem(bestSlot, backpack.getStackInSlot(bestSlot).getCount(), false);
        if (!hand.isEmpty()) {
            backpack.setStackInSlot(bestSlot, hand);
        }
        maid.setItemInHand(InteractionHand.MAIN_HAND, tool);
    }

    /** 她能站在这一格上吗：自己和头顶是空的，脚下有东西接着。 */
    private static boolean standable(Level level, BlockPos pos) {
        if (!level.isLoaded(pos) || !level.isLoaded(pos.above())) {
            return false;
        }
        BlockState feet = level.getBlockState(pos);
        BlockState head = level.getBlockState(pos.above());
        if (!feet.isAir() && !feet.canBeReplaced()) {
            return false;
        }
        if (!head.isAir() && !head.canBeReplaced()) {
            return false;
        }
        return !level.getBlockState(pos.below()).getCollisionShape(level, pos.below()).isEmpty();
    }

    /** 从落脚点的眼睛出发，够得着而且第一眼看到的就是目标。 */
    private static boolean reachableFrom(Level level, BlockPos stand, BlockPos target, EntityMaid maid) {
        Vec3 eye = Vec3.atBottomCenterOf(stand).add(0.0, maid.getEyeHeight(), 0.0);
        if (eye.distanceTo(Vec3.atCenterOf(target)) > EYE_REACH) {
            return false;
        }
        return inSight(level, eye, target, maid);
    }

    /**
     * 视线检查。草、花这类没有碰撞体积的方块用轮廓射线，否则它们永远「看不见」。
     *
     * <p>起点若已经在目标方块内部（比如站在方块里），射线会直接命中它，算看得见；
     * 这是想要的结果，挖自己脚边的东西本来就该行。
     */
    private static boolean inSight(Level level, Vec3 eye, BlockPos target, EntityMaid maid) {
        BlockState state = level.getBlockState(target);
        ClipContext.Block mode = state.getCollisionShape(level, target).isEmpty()
                ? ClipContext.Block.OUTLINE
                : ClipContext.Block.COLLIDER;
        BlockHitResult hit = level.clip(new ClipContext(eye, Vec3.atCenterOf(target), mode, ClipContext.Fluid.NONE, maid));
        return hit.getType() == HitResult.Type.BLOCK && hit.getBlockPos().equals(target);
    }
}
