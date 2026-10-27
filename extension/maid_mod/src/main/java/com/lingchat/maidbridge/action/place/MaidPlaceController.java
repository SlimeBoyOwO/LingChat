package com.lingchat.maidbridge.action.place;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.action.mine.MineRules;
import com.lingchat.maidbridge.action.mine.MineTargets;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.maid.MaidInventory;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.item.ItemReporter;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;

/**
 * 放一块方块的执行者：走到够得着的地方，像玩家那样按一下，扣掉一块材料。
 *
 * <p>与挖掘同一个节奏：单槽位、每 tick 推进、收尾推一条事件。放一块很快，费时间的是
 * 走过去；走不到、那一格被占了、材料没了都如实收尾。
 */
public final class MaidPlaceController {
    /** 走路卡住这么久还没到落脚点，就认为这条路走不通。 */
    private static final int STUCK_TICKS = 80;
    /** 从受理到放下最多等这么久（约一分钟）：放一块不该拖过挖一块。 */
    private static final int PLACE_TIMEOUT_TICKS = 1200;
    /** 站到离目标这么近就能放（底面中心距离平方）。 */
    private static final double AT_STAND = 4.0;

    private static final class Run {
        final String maidId;
        final ServerLevel level;
        final BlockPos target;
        final String blockId;
        final String itemId;
        final Item item;
        final BlockPos preferredSupport;
        final Direction preferredFace;
        final Float yaw;
        BlockPos stand;
        int ticks;

        Run(EntityMaid maid, BlockPos target, String blockId, String itemId, Block block,
                BlockPos preferredSupport, Direction preferredFace, Float yaw) {
            this.maidId = maid.getStringUUID();
            this.level = (ServerLevel) maid.level();
            this.target = target;
            this.blockId = blockId;
            this.itemId = itemId;
            this.item = block.asItem();
            this.preferredSupport = preferredSupport;
            this.preferredFace = preferredFace;
            this.yaw = yaw;
        }
    }

    private static Run current;

    private MaidPlaceController() {
    }

    /** 受理一次放置：目标、方块与材料已经由 handler 核过，这里只记下来。 */
    public static void start(
            EntityMaid maid, BlockPos target, String blockId, String itemId, Block block,
            BlockPos preferredSupport, Direction preferredFace, Float yaw) {
        current = new Run(maid, target, blockId, itemId, block, preferredSupport, preferredFace, yaw);
        MaidActuator.beginWalk(maid);
    }

    /** 这只女仆正在被我带着放：跟随行为给长任务让路时用。 */
    public static boolean isRunning(EntityMaid maid) {
        Run run = current;
        return run != null && maid != null && run.maidId.equals(maid.getStringUUID());
    }

    /** 被别的指令顶掉，或停服时收手。放置没有半成品，不广播。 */
    public static void cancel(MinecraftServer server) {
        Run run = current;
        current = null;
        if (run == null) {
            return;
        }
        EntityMaid maid = MaidQueries.findMaidById(server, run.maidId);
        if (maid == null) {
            return;
        }
        MaidActuator.stopWalking(maid);
        MaidActuator.stopLooking(maid);
    }

    /** 昏迷打断：收手并给插件一个收尾，别让它一直等。 */
    public static void cancelForUnconscious(MinecraftServer server) {
        Run run = current;
        if (run == null) {
            return;
        }
        current = null;
        EntityMaid maid = MaidQueries.findMaidById(server, run.maidId);
        if (maid != null) {
            MaidActuator.stopWalking(maid);
            MaidActuator.stopLooking(maid);
        }
        broadcast(run, "failed", "unconscious", "她昏过去了");
    }

    /** 每个服务端 tick 推进一次（主线程）。 */
    public static void tick(MinecraftServer server) {
        Run run = current;
        if (run == null) {
            return;
        }
        EntityMaid maid = MaidQueries.findMaidById(server, run.maidId);
        if (maid == null || !maid.isAlive() || maid.level() != run.level) {
            current = null;
            return;
        }
        if (MaidQueries.carriedByPlayer(maid)) {
            abandon(run, maid, "carried", "玩家把你抱起来了，先不放了");
            return;
        }
        if (maid.isSleeping() || maid.isMaidInSittingPose() || maid.isPassenger()) {
            MaidActuator.standUp(maid);
        }
        if (++run.ticks > PLACE_TIMEOUT_TICKS) {
            abandon(run, maid, "timeout", "走了这么久还没放下，先算了");
            return;
        }
        if (!run.level.getBlockState(run.target).canBeReplaced()) {
            abandon(run, maid, "not_replaceable", "那一格已经被别的东西占了");
            return;
        }
        if (otherEntityInside(run, maid)) {
            abandon(run, maid, "occupied", "那一格里站着别的人或动物");
            return;
        }
        if (selfInside(run, maid)) {
            // 她自己就站在要放的那一格：先挪到旁边，不然会把自己埋进去
            walk(run, maid);
            return;
        }
        if (!withinReach(maid, run.target)) {
            walk(run, maid);
            return;
        }
        place(run, maid);
    }

    private static void place(Run run, EntityMaid maid) {
        ItemStack stack = new ItemStack(run.item);
        BlockPlaceContext context = Placement.context(
                maid, stack, run.target, run.preferredSupport, run.preferredFace, run.yaw);
        if (context == null) {
            abandon(run, maid, "no_support", "那一格旁边没有能贴着的面了");
            return;
        }
        if (!Placement.inSight(maid, context)) {
            if (run.stand == null || !atStand(run, maid)) {
                walk(run, maid);
                return;
            }
            abandon(run, maid, "no_sight", "中间隔着东西，够不到那一格");
            return;
        }
        if (!MaidInventory.has(maid, run.item)) {
            abandon(run, maid, "no_item", "她身上没有这种方块了");
            return;
        }
        MaidActuator.stopWalking(maid);
        MaidActuator.lookAtPos(maid, run.target);
        InteractionResult result = ((BlockItem) run.item).place(context);
        if (!result.consumesAction()) {
            abandon(run, maid, "place_failed", "那个方块在这一格放不住");
            return;
        }
        if (MaidInventory.takeOne(maid, run.item)) {
            // 少掉的是她放下去的那一块，账上记一笔：不然背包差分把这次消耗说成主人从她身上拿走了
            ItemReporter.markRemoval(run.itemId, 1);
        }
        maid.swing(InteractionHand.MAIN_HAND);
        finish(run, maid, "done", "", "");
    }

    private static void walk(Run run, EntityMaid maid) {
        if (run.stand == null) {
            run.stand = MineTargets.standForPlacement(maid, run.target);
        }
        if (run.stand == null && selfInside(run, maid)) {
            abandon(run, maid, "occupied", "她站的那一格就是要放的那一格，旁边又没地方挪");
            return;
        }
        BlockPos dest = run.stand != null ? run.stand : run.target;
        MaidActuator.walkToPos(maid, dest, 0);
        if (run.ticks > STUCK_TICKS
                && maid.getBrain().hasMemoryValue(MemoryModuleType.CANT_REACH_WALK_TARGET_SINCE)) {
            abandon(run, maid, "unreachable", "那一格她走不到跟前");
        }
    }

    private static boolean atStand(Run run, EntityMaid maid) {
        return run.stand != null
                && maid.position().distanceToSqr(Vec3.atBottomCenterOf(run.stand)) <= AT_STAND;
    }

    private static boolean withinReach(EntityMaid maid, BlockPos target) {
        return maid.getEyePosition().distanceTo(Vec3.atCenterOf(target)) <= MineRules.REACH + 0.5;
    }

    private static boolean selfInside(Run run, EntityMaid maid) {
        return maid.getBoundingBox().intersects(new AABB(run.target));
    }

    private static boolean otherEntityInside(Run run, EntityMaid maid) {
        for (Entity entity : run.level.getEntities(maid, new AABB(run.target))) {
            if (entity.isAlive() && !entity.isSpectator()) {
                return true;
            }
        }
        return false;
    }

    private static void abandon(Run run, EntityMaid maid, String reasonCode, String reason) {
        finish(run, maid, "failed", reasonCode, reason);
    }

    private static void finish(
            Run run, EntityMaid maid, String status, String reasonCode, String reason) {
        current = null;
        MaidActuator.stopWalking(maid);
        MaidActuator.stopLooking(maid);
        broadcast(run, status, reasonCode, reason);
    }

    private static void broadcast(Run run, String status, String reasonCode, String reason) {
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge == null || !bridge.hasClients()) {
            return;
        }
        JsonObject data = new JsonObject();
        data.addProperty("event_type", "maid_place_done");
        data.addProperty("maid_id", run.maidId);
        data.addProperty("status", status);
        data.addProperty("block", run.blockId);
        data.addProperty("item", run.itemId);
        data.addProperty("reason_code", reasonCode == null ? "" : reasonCode);
        data.addProperty("reason", reason == null ? "" : reason);
        bridge.broadcastEvent(data);
    }
}
