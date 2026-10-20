package com.lingchat.maidbridge.action.mine;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.maid.MaidQueries;
import java.util.List;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.sounds.SoundSource;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;

/**
 * 挖方块指令的执行者：走到挑好的落脚点、朝着方块一下一下地挖，挖穿了就捡起掉落物。
 *
 * <p>和攻击循环一样，不挂车万女仆的任务系统：她此刻在种地、钓鱼还是闲着，玩家喊一句
 * 「把这块挖了」都该动。也不走 {@code destroyBlock} 的瞬破——方块按硬度与工具慢慢裂开，
 * 玩家看得见进度，拿着木镐挖黑曜石也不会突然变快。
 *
 * <p>单槽位：新的挖掘令顶掉旧的那次。动作、攻击、换工作模式这些指令也会把它收掉。
 * 只在服务端主线程上跑（请求经 MessageRouter 排队、推进挂在 tick 事件上），状态不需要同步。
 */
public final class MaidMineController {
    /** 两次挥镐之间隔这么多 tick：不是冷却，只是别让她胳膊甩成电风扇。 */
    private static final int SWING_INTERVAL = 5;
    /** 走路卡住这么久还没到落脚点，就认为这条路走不通。 */
    private static final int STUCK_TICKS = 80;

    private static final class Run {
        final String maidId;
        final ServerLevel level;
        final String blockId;
        /** {@link #blockId} 展开后的一族方块：泛称（原木）与深层矿石都在里面。 */
        final Set<Block> blocks;
        final int wanted;
        /** 玩家准星指着的那一片；为 null 时按她自己的位置找。 */
        final BlockPos anchor;
        int mined;
        BlockPos target;
        BlockPos stand;
        float progress;
        int ticks;
        int swingTicks;
        int crack = -1;

        Run(EntityMaid maid, BlockPos target, String blockId, int wanted, BlockPos anchor) {
            this.maidId = maid.getStringUUID();
            this.level = (ServerLevel) maid.level();
            this.blockId = blockId;
            this.blocks = MineTargets.targets(blockId);
            this.wanted = wanted;
            this.target = target;
            this.anchor = anchor;
        }
    }

    private static Run current;

    private MaidMineController() {
    }

    /** 受理一次挖掘：目标与块数已经由 handler 定好，这里只记下来。 */
    public static void start(EntityMaid maid, BlockPos target, String blockId, int wanted, BlockPos anchor) {
        current = new Run(maid, target, blockId, wanted, anchor);
        MaidActuator.beginWalk(maid);
    }

    /** 被别的指令顶掉，或停服时清干净。 */
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
        clearCracks(run, maid);
        MaidActuator.stopWalking(maid);
        MaidActuator.stopLooking(maid);
    }

    /** 这只女仆正在被我们带着挖：跟随行为给长任务让路时用。 */
    public static boolean isRunning(EntityMaid maid) {
        Run run = current;
        return run != null && maid != null && run.maidId.equals(maid.getStringUUID());
    }

    /** 每个服务端 tick 推进一次（主线程）。 */
    public static void tick(MinecraftServer server) {
        Run run = current;
        if (run == null) {
            return;
        }
        EntityMaid maid = MaidQueries.findMaidById(server, run.maidId);
        if (maid == null || !maid.isAlive() || maid.level() != run.level) {
            // 她没了（死在别处、区块卸载）：没有可交代的，死亡与受伤旁白会说明
            current = null;
            return;
        }
        if (MaidQueries.carriedByPlayer(maid)) {
            abandon(run, maid, "carried", "玩家把你抱起来了，先不挖了");
            return;
        }
        if (maid.isSleeping() || maid.isMaidInSittingPose() || maid.isPassenger()) {
            MaidActuator.standUp(maid);
        }
        if (run.target == null && !nextTarget(run, maid)) {
            return;
        }
        run.ticks++;
        if (run.ticks > MineRules.TARGET_TIMEOUT_TICKS) {
            abandon(run, maid, "timeout", "挖了这么久还没挖到，先算了");
            return;
        }
        BlockState state = run.level.getBlockState(run.target);
        if (state.isAir() || !run.blocks.contains(state.getBlock())) {
            // 别人先挖掉了，或者方块被换成了别的：这块不算数，再找一块
            clearCracks(run, maid);
            run.target = null;
            run.progress = 0.0F;
            run.ticks = 0;
            return;
        }
        if (!MineTargets.canMine(maid, run.target)) {
            walkToTarget(run, maid);
            return;
        }
        MaidActuator.stopWalking(maid);
        MaidActuator.lookAtPos(maid, run.target);
        MineTargets.equipBestTool(maid, state);
        mine(run, maid, state);
    }

    private static void walkToTarget(Run run, EntityMaid maid) {
        if (run.stand == null) {
            run.stand = MineTargets.standFor(maid, run.target);
        }
        if (run.stand == null) {
            abandon(run, maid, "unreachable", "那块方块她走不到跟前");
            return;
        }
        MaidActuator.walkToPos(maid, run.stand, 0);
        if (run.ticks > STUCK_TICKS
                && maid.getBrain().hasMemoryValue(MemoryModuleType.CANT_REACH_WALK_TARGET_SINCE)) {
            abandon(run, maid, "unreachable", "那块方块她走不到跟前");
        }
    }

    private static void mine(Run run, EntityMaid maid, BlockState state) {
        float hardness = state.getDestroySpeed(run.level, run.target);
        ItemStack tool = maid.getMainHandItem();
        // 工具中途挖断了：这个方块再挖下去也掉不出东西，别白费力气
        if (state.requiresCorrectToolForDrops() && !tool.isCorrectToolForDrops(state)) {
            abandon(run, maid, "wrong_tool", "工具没了，再挖也拿不到东西");
            return;
        }
        float delta = MineRules.progressPerTick(
                hardness, tool.getDestroySpeed(state), tool.isCorrectToolForDrops(state));
        if (delta <= 0.0F) {
            abandon(run, maid, "not_mineable", "这个方块挖不动");
            return;
        }
        run.progress = Math.min(1.0F, run.progress + delta);
        if (--run.swingTicks <= 0) {
            run.swingTicks = SWING_INTERVAL;
            maid.swing(InteractionHand.MAIN_HAND);
            run.level.playSound(
                    (Entity) null,
                    run.target,
                    state.getSoundType().getHitSound(),
                    SoundSource.BLOCKS,
                    0.25F,
                    state.getSoundType().getPitch() * 0.5F);
        }
        int stage = MineRules.crackStage(run.progress);
        if (stage != run.crack) {
            run.level.destroyBlockProgress(maid.getId(), run.target, stage);
            run.crack = stage;
        }
        if (run.progress < 1.0F) {
            return;
        }
        clearCracks(run, maid);
        BlockPos broken = run.target;
        if (!maid.destroyBlock(broken, true)) {
            abandon(run, maid, "protected", "这个方块不能让她挖");
            return;
        }
        // 与玩家挖方块一样扣一点耐久：挖断了就换背包里下一把
        ItemStack used = maid.getMainHandItem();
        if (!used.isEmpty()) {
            used.hurtAndBreak(1, maid, EquipmentSlot.MAINHAND);
        }
        run.mined++;
        run.target = null;
        run.stand = null;
        run.progress = 0.0F;
        run.ticks = 0;
        if (run.mined >= run.wanted) {
            finish(run, maid, "done", "", "");
        }
    }

    /** 找下一块同类方块。有锚点时只在那一带找，找不到就收尾。 */
    private static boolean nextTarget(Run run, EntityMaid maid) {
        BlockPos center = run.anchor != null ? run.anchor : maid.blockPosition();
        int radius = run.anchor != null ? MineRules.ANCHOR_RANGE : MineRules.FIND_RADIUS;
        List<BlockPos> candidates =
                MineTargets.nearest(maid, center, run.blockId, radius, MineRules.FIND_LIMIT);
        for (BlockPos candidate : candidates) {
            // 受保护的方块跳过：第一块验过，后面每块也得验
            if (!maid.canDestroyBlock(candidate)) {
                continue;
            }
            BlockPos stand = MineTargets.standFor(maid, candidate);
            if (stand == null) {
                continue;
            }
            run.target = candidate;
            run.stand = stand;
            run.progress = 0.0F;
            run.ticks = 0;
            MaidActuator.beginWalk(maid);
            return true;
        }
        String where = run.anchor != null ? "你指的那片" : "附近";
        if (run.mined > 0) {
            finish(run, maid, "partial", "no_target", where + "没有能挖的同类方块了");
        } else {
            finish(run, maid, "failed", "no_target", where + "没有能挖到的这种方块");
        }
        return false;
    }

    private static void abandon(Run run, EntityMaid maid, String reasonCode, String reason) {
        if (run.mined > 0) {
            finish(run, maid, "partial", reasonCode, reason);
        } else {
            finish(run, maid, "failed", reasonCode, reason);
        }
    }

    private static void finish(Run run, EntityMaid maid, String status, String reasonCode, String reason) {
        clearCracks(run, maid);
        current = null;
        MaidActuator.stopWalking(maid);
        MaidActuator.stopLooking(maid);
        broadcast(run, status, reasonCode, reason);
    }

    private static void clearCracks(Run run, EntityMaid maid) {
        if (run.target != null && run.crack >= 0) {
            run.level.destroyBlockProgress(maid.getId(), run.target, -1);
            run.crack = -1;
        }
    }

    private static void broadcast(Run run, String status, String reasonCode, String reason) {
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge == null || !bridge.hasClients()) {
            return;
        }
        JsonObject data = new JsonObject();
        data.addProperty("event_type", "maid_mine_done");
        data.addProperty("maid_id", run.maidId);
        data.addProperty("status", status);
        data.addProperty("block", run.blockId);
        data.addProperty("mined", run.mined);
        data.addProperty("wanted", run.wanted);
        data.addProperty("reason_code", reasonCode == null ? "" : reasonCode);
        data.addProperty("reason", reason == null ? "" : reason);
        bridge.broadcastEvent(data);
    }
}
