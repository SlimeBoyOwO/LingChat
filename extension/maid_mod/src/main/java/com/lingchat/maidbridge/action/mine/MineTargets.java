package com.lingchat.maidbridge.action.mine;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.maid.MaidInventory;
import com.lingchat.maidbridge.perception.spatial.LocalTerrain;
import com.lingchat.maidbridge.perception.spatial.SightLine;
import com.lingchat.maidbridge.perception.spatial.TerrainMemory;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.TagKey;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
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
    private static final int PATH_CHECKS = 8;

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
     * 以 {@code center} 为中心找这一种方块，按离中心的远近排好。
     *
     * <p>排序看的是扫描中心，不是她的身体：锚点时那里是玩家指着的那片，连挖时是刚挖掉的
     * 那一块，只有第一块才是她自己站的地方。这样砍树会顺着树干往上接，而不是跳到旁边
     * 离脚下更近的另一棵树。
     *
     * <p>只做粗筛（是不是这一族方块、能不能挖、在不在活动范围内），落脚点留给真正轮到的
     * 那几块再验：一圈几千个方块里逐个算路径是算不起的。
     *
     * <p>世界只读一遍：范围交给 {@link LocalTerrain}，之后所有判断都在这份快照上做。
     */
    public static List<BlockPos> nearest(
            EntityMaid maid, BlockPos center, String blockId, int radius, int limit) {
        Set<Block> targets = targets(blockId);
        List<BlockPos> found = new ArrayList<>();
        if (targets.isEmpty() || !(maid.level() instanceof ServerLevel level)) {
            return found;
        }
        // 竖直上下各 8 格：连挖时扫描中心是她刚挖掉的那块，窗口跟着树干一起往上走，
        // 所以不用把范围撑大——真正卡人的是落脚点，见 standFor。
        int vertical = Math.min(radius, 8);
        LocalTerrain terrain = LocalTerrain.read(level, center, radius, vertical, vertical);
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        terrain.forEachLoaded((x, y, z, state) -> {
            if (state.isAir() || !targets.contains(state.getBlock())) {
                return;
            }
            // 水与岩浆不是「挖」出来的：让她们去舀是另一回事
            if (state.getBlock() instanceof LiquidBlock) {
                return;
            }
            pos.set(x, y, z);
            if (!MineRules.mineable(state.getDestroySpeed(level, pos))) {
                return;
            }
            if (maid.hasRestriction() && !maid.isWithinRestriction(pos)) {
                return;
            }
            found.add(pos.immutable());
        });
        found.sort(Comparator.comparingDouble(block -> center.distSqr(block)));
        return found.size() > limit ? new ArrayList<>(found.subList(0, limit)) : found;
    }

    /**
     * 整团同种方块：从 {@code seed} 出发按六面相连收，最多 {@code limit} 格。
     *
     * <p>与「看一眼周围」的成团口径一致（同一种方块、六面相连），但这里要的是能拆的格子，
     * 所以顺路跳过她活动范围外的。未加载的邻居不往里扩：那边还有没有，她也不知道。
     */
    public static Cells cluster(EntityMaid maid, BlockPos seed, String blockId, int limit) {
        List<BlockPos> cells = new ArrayList<>();
        if (!(maid.level() instanceof ServerLevel level) || limit <= 0) {
            return new Cells(cells, false);
        }
        Set<Block> targets = targets(blockId);
        if (targets.isEmpty()) {
            return new Cells(cells, false);
        }
        Set<BlockPos> seen = new HashSet<>();
        ArrayDeque<BlockPos> queue = new ArrayDeque<>();
        seen.add(seed);
        queue.add(seed);
        boolean truncated = false;
        while (!queue.isEmpty()) {
            if (cells.size() >= limit) {
                truncated = true;
                break;
            }
            BlockPos pos = queue.poll();
            BlockState state = level.getBlockState(pos);
            if (state.isAir() || !targets.contains(state.getBlock())) {
                continue;
            }
            if (maid.hasRestriction() && !maid.isWithinRestriction(pos)) {
                continue;
            }
            cells.add(pos);
            for (Direction side : Direction.values()) {
                BlockPos next = pos.relative(side);
                if (level.isLoaded(next) && seen.add(next)) {
                    queue.add(next);
                }
            }
        }
        return new Cells(cells, truncated);
    }

    /** 一团方块：格子清单（BFS 顺序），以及是不是数到上限还没数完。 */
    public record Cells(List<BlockPos> cells, boolean truncated) {
    }

    /**
     * 站哪能挖到这一块：站得下、够得着、看得见，而且寻路能走到。找不到给 null。
     *
     * <p>准入点选**离目标最近**的那一处：她先贴到方块跟前再动手，不站在半路上隔空挥镐。
     * 竖直往下多看几格（-6）：树干上方的方块只能站在树底下往上挖，只在目标周围 ±2 找的话，
     * 高处那块就会「没有落脚点」，整棵树砍到一半就没了下文。
     *
     * <p>看得见按准星射线算：视线先碰到的必须是目标方块本身，隔着一堵墙挖是不行的。
     * 草、花这类没有碰撞体积的方块用轮廓射线，否则它们永远「看不见」。
     */
    public static BlockPos standFor(EntityMaid maid, BlockPos target) {
        if (!(maid.level() instanceof ServerLevel level)) {
            return null;
        }
        List<BlockPos> candidates = standCandidates(maid, target);
        int checks = Math.min(candidates.size(), PATH_CHECKS);
        for (int i = 0; i < checks; i++) {
            BlockPos stand = candidates.get(i);
            if (!reachableFrom(level, stand, target, maid)) {
                continue;
            }
            // 已经站在跟前就不用再问寻路了
            if (stand.closerToCenterThan(maid.position(), 2.0) || maid.canPathReach(stand)) {
                return stand;
            }
        }
        return null;
    }

    /**
     * 放置用的落脚点：伸手够得着要放的那一格就行。
     *
     * <p>不用「视线落到目标」那一条：目标是空格子，射线打不到它。中间隔没隔着东西由
     * 放置上下文按支撑面另判。
     */
    public static BlockPos standForPlacement(EntityMaid maid, BlockPos target) {
        if (!(maid.level() instanceof ServerLevel level)) {
            return null;
        }
        List<BlockPos> candidates = standCandidates(maid, target);
        int checks = Math.min(candidates.size(), PATH_CHECKS);
        for (int i = 0; i < checks; i++) {
            BlockPos stand = candidates.get(i);
            Vec3 eye = Vec3.atBottomCenterOf(stand).add(0.0, maid.getEyeHeight(), 0.0);
            if (eye.distanceTo(Vec3.atCenterOf(target)) > EYE_REACH) {
                continue;
            }
            if (stand.closerToCenterThan(maid.position(), 2.0) || maid.canPathReach(stand)) {
                return stand;
            }
        }
        return null;
    }

    /** 目标周围站得下、在活动范围里的格子，按离目标远近排好。 */
    private static List<BlockPos> standCandidates(EntityMaid maid, BlockPos target) {
        ServerLevel level = (ServerLevel) maid.level();
        List<BlockPos> candidates = new ArrayList<>();
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        for (int dx = -2; dx <= 2; dx++) {
            for (int dz = -2; dz <= 2; dz++) {
                for (int dy = -6; dy <= 2; dy++) {
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
                    candidates.add(stand);
                }
            }
        }
        candidates.sort(Comparator.comparingDouble(stand -> stand.distSqr(target)));
        return candidates;
    }

    /** 目标周围有没有能站人的空格：只看空间，不问寻路。失败回执用它说「站不下」。 */
    public static boolean standRoom(EntityMaid maid, BlockPos target) {
        Level level = maid.level();
        for (int dx = -2; dx <= 2; dx++) {
            for (int dz = -2; dz <= 2; dz++) {
                for (int dy = -2; dy <= 2; dy++) {
                    if (dx == 0 && dy == 0 && dz == 0) {
                        continue;
                    }
                    if (standable(level, target.offset(dx, dy, dz))) {
                        return true;
                    }
                }
            }
        }
        return false;
    }

    /**
     * 挡在她和目标之间的那一块：从她眼睛打一条射线过去，先撞上的如果不是目标，就是它。
     *
     * <p>矿脉、树心那种埋在里面的方块，从任何落脚点都看不见；要么放弃，要么把挡路的挖穿。
     * 这里只把「挡路的是哪一块」找出来，挖不挖、够不够得着由 {@link MaidMineController}
     * 每 tick 判。看得见目标时给 null。
     */
    public static BlockPos blocking(EntityMaid maid, BlockPos target) {
        if (!(maid.level() instanceof ServerLevel level)) {
            return null;
        }
        BlockState state = level.getBlockState(target);
        ClipContext.Block mode = state.getCollisionShape(level, target).isEmpty()
                ? ClipContext.Block.OUTLINE
                : ClipContext.Block.COLLIDER;
        BlockHitResult hit = level.clip(new ClipContext(
                maid.getEyePosition(), Vec3.atCenterOf(target), mode, ClipContext.Fluid.NONE, maid));
        if (hit.getType() != HitResult.Type.BLOCK) {
            return null;
        }
        BlockPos hitPos = hit.getBlockPos();
        if (hitPos.equals(target)) {
            return null;
        }
        BlockState blocker = level.getBlockState(hitPos);
        if (blocker.isAir() || blocker.getBlock() instanceof LiquidBlock) {
            return null;
        }
        return hitPos;
    }

    /**
     * 她知道这一格是什么吗：此刻看得见，或者半分钟内真的看见过。
     *
     * <p>格子寻址的门槛。模型指的那一格若不在她的认知里，那多半是它自己编的坐标，
     * 直接拒绝比让她照着想象中的坐标动手更像「她只知道自己看见过的」。
     */
    public static boolean sheKnows(EntityMaid maid, BlockPos pos, BlockState state) {
        Level level = maid.level();
        if (SightLine.sees(level, maid.getEyePosition(), pos, maid)) {
            return true;
        }
        TerrainMemory.Remembered remembered =
                TerrainMemory.recall(maid, blockId(state), level.getGameTime());
        return remembered != null && remembered.pos().equals(pos);
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
        // 副手拿着的工具也算「她身上有」：车万女仆的界面里两只手都能放东西
        if (maid.getOffhandItem().isCorrectToolForDrops(state)) {
            return true;
        }
        CombinedInvWrapper backpack = maid.getAvailableBackpackInv();
        for (int slot = 0; slot < backpack.getSlots(); slot++) {
            if (slot == MaidInventory.HEADWEAR_SLOT) {
                continue;
            }
            if (backpack.getStackInSlot(slot).isCorrectToolForDrops(state)) {
                return true;
            }
        }
        return false;
    }

    /**
     * 没有合适工具时的说明：她要什么、现在拿的是什么、地上有没有现成的。
     *
     * <p>只说「没有工具」会让模型反复重试——它明明看见主人给过一把镐子。所以这里把
     * 「木镐挖不动铜矿石」与「石镐就在脚边地上，得放进她的背包」分开说清楚。
     */
    public static String missingToolReason(EntityMaid maid, BlockState state) {
        String block = state.getBlock().getName().getString();
        String need = neededTool(state);
        String held = maid.getMainHandItem().isEmpty()
                ? ""
                : maid.getMainHandItem().getHoverName().getString();
        StringBuilder reason = new StringBuilder();
        if (held.isEmpty()) {
            reason.append("她手上和背包里都没有能挖").append(block).append("的工具");
        } else {
            reason.append("她手上只有").append(held).append("，挖").append(block).append("不掉东西");
        }
        if (!need.isEmpty()) {
            reason.append("，需要").append(need).append("或更好的");
        }
        String ground = groundToolName(maid, state);
        if (!ground.isEmpty()) {
            reason.append("；脚边地上有一把").append(ground).append("，得先放进她的背包");
        }
        return reason.toString();
    }

    /** 能挖这一块的最弱一件原版工具（石镐、铁镐这类）；判不出来给空串。 */
    private static String neededTool(BlockState state) {
        for (Item item : new Item[] {
                Items.WOODEN_PICKAXE, Items.STONE_PICKAXE, Items.IRON_PICKAXE,
                Items.DIAMOND_PICKAXE, Items.NETHERITE_PICKAXE}) {
            ItemStack stack = new ItemStack(item);
            if (stack.isCorrectToolForDrops(state)) {
                return stack.getHoverName().getString();
            }
        }
        ItemStack shears = new ItemStack(Items.SHEARS);
        return shears.isCorrectToolForDrops(state) ? shears.getHoverName().getString() : "";
    }

    /** 她脚边地上有没有能挖这一块的工具（玩家丢在地上的那些）。 */
    private static String groundToolName(EntityMaid maid, BlockState state) {
        if (!(maid.level() instanceof ServerLevel level)) {
            return "";
        }
        double reach = 8.0;
        for (ItemEntity entity : level.getEntitiesOfClass(
                ItemEntity.class, maid.getBoundingBox().inflate(reach), e -> e.isAlive())) {
            ItemStack stack = entity.getItem();
            if (!stack.isEmpty() && stack.isCorrectToolForDrops(state)) {
                return stack.getHoverName().getString();
            }
        }
        return "";
    }

    /**
     * 把背包里挖这一块最快的工具换到手上；手上本来就是最好的就不动。
     *
     * <p>只比较「正确工具」之间的速度：剑挖树叶再快也不该拿去挖石头，它掉不了东西。
     * 头饰栏不是存货，那一格里的东西拿不下来。
     */
    public static void equipBestTool(EntityMaid maid, BlockState state) {
        ItemStack hand = maid.getMainHandItem();
        boolean handCorrect = hand.isCorrectToolForDrops(state);
        float bestSpeed = handCorrect ? hand.getDestroySpeed(state) : -1.0F;
        // 副手拿着更快的正确工具就先与主手对调，之后再从背包里找更好的
        ItemStack offhand = maid.getOffhandItem();
        if (!offhand.isEmpty() && offhand.isCorrectToolForDrops(state)
                && offhand.getDestroySpeed(state) > bestSpeed) {
            bestSpeed = offhand.getDestroySpeed(state);
            maid.setItemInHand(InteractionHand.OFF_HAND, hand);
            maid.setItemInHand(InteractionHand.MAIN_HAND, offhand);
            hand = maid.getMainHandItem();
        }
        CombinedInvWrapper backpack = maid.getAvailableBackpackInv();
        int bestSlot = -1;
        for (int slot = 0; slot < backpack.getSlots(); slot++) {
            if (slot == MaidInventory.HEADWEAR_SLOT) {
                continue;
            }
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
        return SightLine.sees(level, eye, target, maid);
    }
}
