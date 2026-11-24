package com.lingchat.maidbridge.action.nav;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.place.Placement;
import com.lingchat.maidbridge.maid.MaidInventory;
import com.lingchat.maidbridge.perception.item.ItemReporter;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.BlockStateProperties;
import net.minecraft.world.phys.AABB;
import net.neoforged.neoforge.items.wrapper.CombinedInvWrapper;

/**
 * 垫脚账本：寻路为跨过缺口、堆高、落脚临时放下的方块，全记在这里。
 *
 * <p>谁放的、为什么要放不用记，只记放的是什么、什么时候拆。她走过之后由 {@link #clean}
 * 逐块拆掉；收工或被打断时 {@link #clearNow} 把剩下的拆净，生存模式下把方块还回她背包。
 */
public final class ScaffoldBook {
    private final Map<BlockPos, Block> blocks = new LinkedHashMap<>();
    /** 这次活里一共垫过多少块，只增不减，给「这一遍有没有进展」当依据。 */
    private int placed;

    public boolean isEmpty() {
        return blocks.isEmpty();
    }

    public int size() {
        return blocks.size();
    }

    public int placed() {
        return placed;
    }

    public boolean contains(BlockPos pos) {
        return blocks.containsKey(pos);
    }

    /** 她脚下踩的是不是自己垫出来的方块。 */
    public boolean standingOn(EntityMaid maid) {
        return blocks.containsKey(maid.blockPosition().below());
    }

    /** 垫脚用的方块：优先她自己背着的整方块；测试模式背包空着时借一块圆石。 */
    public static Item item(EntityMaid maid, boolean consumeMaterials) {
        Item carried = MaidInventory.scaffoldBlock(maid);
        if (carried != null) {
            return carried;
        }
        return consumeMaterials ? null : Blocks.COBBLESTONE.asItem();
    }

    /** 放一块垫脚方块：扣料、记账，她走开后由 {@link #clean} 拆掉。 */
    public boolean put(
            ServerLevel level, EntityMaid maid, BlockPos pos, Item item, boolean consumeMaterials) {
        if (blocks.size() >= ScaffoldRules.MAX_BLOCKS) {
            return false;
        }
        if (consumeMaterials && !MaidInventory.has(maid, item)) {
            return false;
        }
        if (!Terrain.free(level, pos) || Bodies.self(pos, maid) || Bodies.other(level, pos, maid)) {
            return false;
        }
        Block block = ((BlockItem) item).getBlock();
        com.lingchat.maidbridge.action.core.MaidActuator.stopWalking(maid);
        lookToward(maid, pos);
        BlockPlaceContext context =
                Placement.visibleContext(maid, new ItemStack(item), pos, null, null, null);
        if (context == null) {
            return false;
        }
        ((BlockItem) item).place(context);
        if (level.getBlockState(pos).getBlock() != block) {
            return false;
        }
        upright(level, pos);
        if (consumeMaterials && MaidInventory.takeOne(maid, item)) {
            ItemReporter.markRemoval(BuiltInRegistries.ITEM.getKey(item).toString(), 1);
        }
        maid.swing(InteractionHand.MAIN_HAND);
        blocks.put(pos.immutable(), block);
        placed++;
        return true;
    }

    /**
     * 她已经离开的垫脚块就拆掉：留一地毛刺不像话，生存模式下把方块还回她背包。
     *
     * <p>人还踩在上面、或者正在为当前这一趟护着的落脚点附近，先留着；正走在一段路上的
     * 时候也先不拆，否则会拆掉自己脚下的台阶。force 时几样都放开，一块不留。
     */
    public void clean(
            ServerLevel level, EntityMaid maid, BlockPos protect,
            boolean force, boolean busy, boolean consumeMaterials) {
        if (blocks.isEmpty() || (busy && !force)) {
            return;
        }
        AABB body = maid.getBoundingBox().inflate(0.1);
        Iterator<Map.Entry<BlockPos, Block>> iterator = blocks.entrySet().iterator();
        while (iterator.hasNext()) {
            Map.Entry<BlockPos, Block> entry = iterator.next();
            BlockPos pos = entry.getKey();
            BlockState state = level.getBlockState(pos);
            if (state.getBlock() != entry.getValue()) {
                iterator.remove();
                continue;
            }
            if (!force && body.intersects(new AABB(pos))) {
                continue;
            }
            if (!force && protect != null && pos.distSqr(protect) <= 4) {
                continue;
            }
            level.destroyBlock(pos, false);
            if (consumeMaterials) {
                giveBack(maid, entry.getValue().asItem());
            }
            iterator.remove();
        }
    }

    /** 收手：剩下的垫脚块一块不留。 */
    public void clearNow(ServerLevel level, EntityMaid maid, boolean consumeMaterials) {
        clean(level, maid, null, true, false, consumeMaterials);
    }

    /** 把垫脚拆下来的方块塞回她背包；塞不下就掉在她脚下。 */
    private static void giveBack(EntityMaid maid, Item item) {
        ItemStack stack = new ItemStack(item);
        int count = stack.getCount();
        CombinedInvWrapper backpack = maid.getAvailableBackpackInv();
        for (int slot = 0; slot < backpack.getSlots() && !stack.isEmpty(); slot++) {
            if (slot == MaidInventory.HEADWEAR_SLOT) {
                continue;
            }
            stack = backpack.insertItem(slot, stack, false);
        }
        int inserted = count - stack.getCount();
        if (inserted > 0) {
            // 这块本来就该在她身上：背包差分别把它说成主人刚塞进来的
            ItemReporter.suppressHandGain(new ItemStack(item, inserted));
        }
        if (!stack.isEmpty()) {
            Block.popResource(maid.level(), maid.blockPosition(), stack);
        }
    }

    /** 原木这类有朝向轴的方块强制竖着放，不按她点的那一面横过来。 */
    private static void upright(ServerLevel level, BlockPos pos) {
        BlockState state = level.getBlockState(pos);
        if (!state.hasProperty(BlockStateProperties.AXIS)
                || state.getValue(BlockStateProperties.AXIS) == Direction.Axis.Y) {
            return;
        }
        level.setBlock(pos, state.setValue(BlockStateProperties.AXIS, Direction.Axis.Y), 3);
    }

    /** 把身子和头一起转向这一格再动手：转头控制一 tick 只转三十度，现转来不及。 */
    private static void lookToward(EntityMaid maid, BlockPos pos) {
        net.minecraft.world.phys.Vec3 eye = maid.getEyePosition();
        double dx = pos.getX() + 0.5 - eye.x;
        double dy = pos.getY() + 0.5 - eye.y;
        double dz = pos.getZ() + 0.5 - eye.z;
        float yaw = (float) (Math.toDegrees(Math.atan2(dz, dx)) - 90.0);
        float pitch = (float) -Math.toDegrees(Math.atan2(dy, Math.sqrt(dx * dx + dz * dz)));
        maid.setYRot(yaw);
        maid.setYHeadRot(yaw);
        maid.setYBodyRot(yaw);
        maid.setXRot(pitch);
        com.lingchat.maidbridge.action.core.MaidActuator.lookAtPos(maid, pos);
    }
}
