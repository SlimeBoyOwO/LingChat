package com.lingchat.maidbridge.action.mine;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.spatial.TerrainMemory;
import java.util.ArrayList;
import java.util.Comparator;
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
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.Vec3;

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
        /** 区域拆除的待拆清单；null 表示按「同种方块若干块」找。 */
        final List<BlockPos> cells;
        /** 这一片比上限大，只拆了最近的这些。 */
        final boolean truncated;
        int mined;
        /** 已经为够到目标挖穿了几块挡路的方块。 */
        int dug;
        BlockPos target;
        BlockPos stand;
        /** 挡在目标前面的那一块：挖穿它再看下一块。它不是战果，挖掉也不计块数。 */
        BlockPos path;
        /** 上一块挖掉的位置。下一块按离它最近来找，砍树才会顺着树干往上接。 */
        BlockPos lastMined;
        float progress;
        int ticks;
        int swingTicks;
        int crack = -1;
        BlockPos cracking;

        Run(EntityMaid maid, BlockPos target, String blockId, int wanted, BlockPos anchor) {
            this(maid, target, blockId, wanted, anchor, null, false);
        }

        Run(EntityMaid maid, BlockPos target, String blockId, int wanted, BlockPos anchor,
                List<BlockPos> cells, boolean truncated) {
            this.maidId = maid.getStringUUID();
            this.level = (ServerLevel) maid.level();
            this.blockId = blockId;
            this.blocks = MineTargets.targets(blockId);
            this.wanted = wanted;
            this.target = target;
            this.anchor = anchor;
            this.cells = cells;
            this.truncated = truncated;
        }
    }

    private static Run current;
    /** 最近一次由我们挖掉方块的时间与那只女仆：背包差分据此避开自挖的收获。 */
    private static final int RECENT_MINE_TICKS = 120;
    private static String lastMineId = "";
    private static long lastMineTick;

    private MaidMineController() {
    }

    /** 受理一次挖掘：目标与块数已经由 handler 定好，这里只记下来。 */
    public static void start(EntityMaid maid, BlockPos target, String blockId, int wanted, BlockPos anchor) {
        current = new Run(maid, target, blockId, wanted, anchor);
        MaidActuator.beginWalk(maid);
    }

    /**
     * 受理一次区域拆除：格子已经由 handler 挑好，这里按清单逐块推进。
     *
     * <p>{@code blockId} 只在这片方块种类一致时才有值；混着几种就空着，控制器按清单拆，
     * 不再按方块族筛。
     */
    public static void startCells(
            EntityMaid maid, List<BlockPos> cells, boolean truncated, String blockId) {
        current = new Run(maid, null, blockId, cells.size(), null, new ArrayList<>(cells), truncated);
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

    /** 昏迷打断挖掘：收干净并给插件一个明确的收尾，别让它一直等。 */
    public static void cancelForUnconscious(MinecraftServer server) {
        Run run = current;
        if (run == null) {
            return;
        }
        current = null;
        EntityMaid maid = MaidQueries.findMaidById(server, run.maidId);
        if (maid != null) {
            clearCracks(run, maid);
            MaidActuator.stopWalking(maid);
            MaidActuator.stopLooking(maid);
        }
        broadcast(run, maid, run.mined > 0 ? "partial" : "failed", "unconscious", "她昏过去了");
    }

    /** 这只女仆正在被我们带着挖：跟随行为给长任务让路时用。 */
    public static boolean isRunning(EntityMaid maid) {
        Run run = current;
        return run != null && maid != null && run.maidId.equals(maid.getStringUUID());
    }

    /**
     * 她刚挖下的东西进了背包（或者刚挖完还没被采样到）。
     *
     * <p>车万女仆的 {@code destroyBlock} 把掉落直接塞进她的库存，不走拾取事件；背包差分
     * 于是把每一块木头当成「背包里多了」，主人站在旁边还会说成「主人放进来的」——
     * 真机里砍一棵树，旁白连着报了七八次「用户酱往DeepSeek的背包里放了橡木原木」。
     * 这段窗口里的增量都是挖出来的，挖掘收尾那条会自己交代结果。
     */
    public static boolean minedRecently(EntityMaid maid) {
        if (maid == null || lastMineId.isEmpty() || !lastMineId.equals(maid.getStringUUID())) {
            return false;
        }
        return maid.level().getGameTime() - lastMineTick <= RECENT_MINE_TICKS;
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
        if (state.isAir() || !wantedBlock(run, state)) {
            // 别人先挖掉了，或者方块被换成了别的：这块不算数，再找一块
            clearCracks(run, maid);
            run.target = null;
            run.path = null;
            run.progress = 0.0F;
            run.ticks = 0;
            return;
        }
        // 手头有块挡路的就先挖穿它：挖掉之后目标自己会露出来，不必再找落脚点
        if (run.path != null && !digPath(run, maid)) {
            return;
        }
        if (MineTargets.canMine(maid, run.target) && atStand(run, maid)) {
            MaidActuator.stopWalking(maid);
            MaidActuator.lookAtPos(maid, run.target);
            MineTargets.equipBestTool(maid, state);
            mine(run, maid, run.target, state, true);
            return;
        }
        // 够得着但隔着东西：把那块挡路的挖掉，走着走着就通了
        BlockPos blocker = MineTargets.blocking(maid, run.target);
        if (blocker != null && canDigPath(run, maid, blocker)) {
            if (run.dug >= MineRules.PATH_LIMIT) {
                abandon(run, maid, "path_limit", "挡在前面的方块太多，挖不过去");
                return;
            }
            run.path = blocker;
            run.progress = 0.0F;
            run.crack = -1;
            digPath(run, maid);
            return;
        }
        walkToTarget(run, maid);
    }

    /** 这一格现在还算不算这次要拆的：区域清单里什么方块都拆，同种方块只认那一族。 */
    private static boolean wantedBlock(Run run, BlockState state) {
        if (run.cells != null && run.blockId.isEmpty()) {
            return true;
        }
        return run.blocks.contains(state.getBlock());
    }

    /**
     * 挖穿一块挡路的方块。返回真表示这一 tick 已经把活干完了（没有可以接着挖的）。
     *
     * <p>挡路的方块可能是石头、也可能是另一棵树；只要她挖得动、够得着、不是受保护的，
     * 都先挖掉。它不是这次要的方块，挖掉不计入块数。
     */
    private static boolean digPath(Run run, EntityMaid maid) {
        BlockPos path = run.path;
        BlockState state = run.level.getBlockState(path);
        if (state.isAir() || !MineTargets.canMine(maid, path)) {
            // 已经通了（或者她挪开了、看不见了）：这一块不用再管
            run.path = null;
            clearCracks(run, maid);
            run.progress = 0.0F;
            return true;
        }
        MaidActuator.stopWalking(maid);
        MaidActuator.lookAtPos(maid, path);
        MineTargets.equipBestTool(maid, state);
        mine(run, maid, path, state, false);
        return false;
    }

    /** 这一块挖得动吗：够得着、不是受保护方块、硬度正常（挖穿只是为了往里去，掉落无所谓）。 */
    private static boolean canDigPath(Run run, EntityMaid maid, BlockPos blocker) {
        if (!MineTargets.canMine(maid, blocker) || !maid.canDestroyBlock(blocker)) {
            return false;
        }
        BlockState state = run.level.getBlockState(blocker);
        return MineRules.mineable(state.getDestroySpeed(run.level, blocker));
    }

    private static void walkToTarget(Run run, EntityMaid maid) {
        if (run.stand == null) {
            run.stand = MineTargets.standFor(maid, run.target);
        }
        // 没有落脚点就朝方块本身走：埋在墙里的矿、树干高处的木头，走到跟前才谈得上挖穿
        BlockPos dest = run.stand != null ? run.stand : run.target;
        MaidActuator.walkToPos(maid, dest, 0);
        if (run.ticks > STUCK_TICKS
                && maid.getBrain().hasMemoryValue(MemoryModuleType.CANT_REACH_WALK_TARGET_SINCE)) {
            abandon(run, maid, "unreachable", "那块方块她走不到跟前");
        }
    }

    /**
     * 站到挑好的落脚点跟前了吗。
     *
     * <p>够得着不代表该动手：路上刚进交互距离就开挖的话，她会站在离树干好几格的地方挥镐。
     * 差个一两格就开始，免得卡在台阶边上永远差半步。
     */
    private static boolean atStand(Run run, EntityMaid maid) {
        return run.stand == null
                || maid.position().distanceToSqr(Vec3.atBottomCenterOf(run.stand)) <= 4.0;
    }

    private static void mine(Run run, EntityMaid maid, BlockPos pos, BlockState state, boolean counted) {
        float hardness = state.getDestroySpeed(run.level, pos);
        ItemStack tool = maid.getMainHandItem();
        // 工具中途挖断了：这个方块再挖下去也掉不出东西，别白费力气。
        // 挡路的方块不按这条算——挖穿只是为了往里去，掉不掉东西无所谓。
        if (counted && state.requiresCorrectToolForDrops() && !tool.isCorrectToolForDrops(state)) {
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
                    pos,
                    state.getSoundType().getHitSound(),
                    SoundSource.BLOCKS,
                    0.25F,
                    state.getSoundType().getPitch() * 0.5F);
        }
        int stage = MineRules.crackStage(run.progress);
        if (stage != run.crack || !pos.equals(run.cracking)) {
            clearCracks(run, maid);
            run.level.destroyBlockProgress(maid.getId(), pos, stage);
            run.crack = stage;
            run.cracking = pos;
        }
        if (run.progress < 1.0F) {
            return;
        }
        clearCracks(run, maid);
        if (!maid.destroyBlock(pos, true)) {
            abandon(run, maid, "protected", "这个方块不能让她挖");
            return;
        }
        // 挖开一格，贴着的那几格现在看得见了：记进她的认知，台词里才说得出「刚才那边」
        TerrainMemory.reveal(maid, pos, run.level.getGameTime());
        // 与玩家挖方块一样扣一点耐久：挖断了就换背包里下一把
        ItemStack used = maid.getMainHandItem();
        if (!used.isEmpty()) {
            used.hurtAndBreak(1, maid, EquipmentSlot.MAINHAND);
        }
        run.progress = 0.0F;
        run.ticks = 0;
        // 挖穿的挡路方块也进她的背包：这一段里的增量同样是挖出来的，背包差分别报成「玩家给的」
        lastMineId = run.maidId;
        lastMineTick = run.level.getGameTime();
        if (!counted) {
            // 挡路的方块：挖穿就算完成任务，下一 tick 重新看目标还挡不挡着
            run.dug++;
            run.path = null;
            return;
        }
        run.mined++;
        run.lastMined = pos;
        run.target = null;
        run.stand = null;
        if (run.cells != null) {
            run.cells.remove(pos);
            if (run.cells.isEmpty()) {
                finish(run, maid, "done", "", "");
            }
            return;
        }
        if (run.mined >= run.wanted) {
            finish(run, maid, "done", "", "");
        }
    }

    /** 找下一块同类方块。有锚点时只在那一带找，找不到就收尾。 */
    private static boolean nextTarget(Run run, EntityMaid maid) {
        if (run.cells != null) {
            return nextCell(run, maid);
        }
        // 第一块看她站的地方；之后看她刚挖掉的那块——同一棵树、同一片矿脉才连得起来
        BlockPos center = run.anchor != null ? run.anchor
                : run.lastMined != null ? run.lastMined
                : maid.blockPosition();
        int radius = run.anchor != null ? MineRules.ANCHOR_RANGE : MineRules.FIND_RADIUS;
        List<BlockPos> candidates =
                MineTargets.nearest(maid, center, run.blockId, radius, MineRules.FIND_LIMIT);
        // 先找有落脚点的；都埋在墙里就挑最近的一块，走过去把挡路的挖穿
        for (BlockPos candidate : candidates) {
            // 受保护的方块跳过：第一块验过，后面每块也得验
            if (!maid.canDestroyBlock(candidate)) {
                continue;
            }
            BlockPos stand = MineTargets.standFor(maid, candidate);
            if (stand == null) {
                continue;
            }
            return startOn(run, maid, candidate, stand);
        }
        for (BlockPos candidate : candidates) {
            if (maid.canDestroyBlock(candidate)) {
                return startOn(run, maid, candidate, null);
            }
        }
        String where = run.anchor != null ? "你指的那片" : "附近";
        if (run.mined > 0) {
            finish(run, maid, "partial", "no_target", where + "没有能挖的同类方块了");
        } else {
            finish(run, maid, "failed", "no_target", where + "没有能挖到的这种方块");
        }
        return false;
    }

    /** 区域模式：从清单里挑剩下最近的一块，已经没了的格子顺手划掉。 */
    private static boolean nextCell(Run run, EntityMaid maid) {
        BlockPos center = run.lastMined != null ? run.lastMined : maid.blockPosition();
        run.cells.removeIf(pos -> !cellValid(run, maid, pos));
        if (run.cells.isEmpty()) {
            if (run.mined > 0) {
                finish(run, maid, "done", "", "");
            } else {
                finish(run, maid, "failed", "no_target", "清单里的方块都已经不在了");
            }
            return false;
        }
        List<BlockPos> sorted = new ArrayList<>(run.cells);
        sorted.sort(Comparator.comparingDouble(center::distSqr));
        // 与同种方块那条一样：先挑有落脚点的；都埋着就挑最近一块，走过去把挡路的挖穿
        for (BlockPos candidate : sorted) {
            if (!maid.canDestroyBlock(candidate)) {
                continue;
            }
            BlockPos stand = MineTargets.standFor(maid, candidate);
            if (stand == null) {
                continue;
            }
            return startOn(run, maid, candidate, stand);
        }
        for (BlockPos candidate : sorted) {
            if (maid.canDestroyBlock(candidate)) {
                return startOn(run, maid, candidate, null);
            }
        }
        finish(run, maid, run.mined > 0 ? "partial" : "failed", "no_target", "剩下的方块都拆不了了");
        return false;
    }

    /** 清单里的这一格还能不能拆：还在、挖得动、没出活动范围；同种方块模式还要求是那一族。 */
    private static boolean cellValid(Run run, EntityMaid maid, BlockPos pos) {
        if (!run.level.isLoaded(pos)) {
            return false;
        }
        if (maid.hasRestriction() && !maid.isWithinRestriction(pos)) {
            return false;
        }
        BlockState state = run.level.getBlockState(pos);
        if (state.isAir() || state.getBlock() instanceof LiquidBlock) {
            return false;
        }
        if (!MineRules.mineable(state.getDestroySpeed(run.level, pos))) {
            return false;
        }
        return run.blockId.isEmpty() || run.blocks.contains(state.getBlock());
    }

    private static boolean startOn(Run run, EntityMaid maid, BlockPos target, BlockPos stand) {
        run.target = target;
        run.stand = stand;
        run.path = null;
        run.progress = 0.0F;
        run.ticks = 0;
        MaidActuator.beginWalk(maid);
        return true;
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
        broadcast(run, maid, status, reasonCode, reason);
    }

    private static void clearCracks(Run run, EntityMaid maid) {
        if (run.cracking != null && run.crack >= 0) {
            run.level.destroyBlockProgress(maid.getId(), run.cracking, -1);
        }
        run.crack = -1;
        run.cracking = null;
    }

    private static void broadcast(
            Run run, EntityMaid maid, String status, String reasonCode, String reason) {
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
        // 区域拆除：area 区分说法，remaining 是清单里还剩几块，stuck_at 是停下时卡着的那一格
        data.addProperty("area", run.cells != null);
        data.addProperty(
                "remaining",
                run.cells != null ? run.cells.size() : Math.max(0, run.wanted - run.mined));
        data.addProperty("truncated", run.truncated);
        data.addProperty("stuck_at", stuckAt(run, maid));
        data.addProperty("reason_code", reasonCode == null ? "" : reasonCode);
        data.addProperty("reason", reason == null ? "" : reason);
        bridge.broadcastEvent(data);
    }

    /** 收尾时卡着的那一格，相对她脚下：区域拆到一半走不到时，插件能把这句念出来。 */
    private static String stuckAt(Run run, EntityMaid maid) {
        if (maid == null || run.target == null) {
            return "";
        }
        BlockPos feet = maid.blockPosition();
        BlockPos at = run.target;
        return MineRules.formatOffset(
                at.getX() - feet.getX(), at.getY() - feet.getY(), at.getZ() - feet.getZ());
    }
}
