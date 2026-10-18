package com.lingchat.maidbridge.action.core;

import com.github.tartaricacid.touhoulittlemaid.api.task.IMaidTask;
import com.github.tartaricacid.touhoulittlemaid.block.BlockMaidBed;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.github.tartaricacid.touhoulittlemaid.entity.task.TaskIdle;
import com.github.tartaricacid.touhoulittlemaid.init.InitBlocks;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.world.PerceptionDiff;
import com.lingchat.maidbridge.perception.world.PerceptionReader;
import com.lingchat.maidbridge.perception.nearby.SurroundingsDiff;
import com.lingchat.maidbridge.perception.nearby.SurroundingsReader;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.ai.behavior.BlockPosTracker;
import net.minecraft.world.entity.ai.behavior.EntityTracker;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.ai.memory.WalkTarget;
import net.minecraft.world.entity.ai.village.poi.PoiManager;
import net.minecraft.world.entity.ai.village.poi.PoiType;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.BedBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.BedPart;
import net.minecraft.world.phys.Vec3;

/**
 * 让女仆动起来的地方：改她的状态、写她的寻路记忆、替她找床。
 *
 * <p>模组里碰游戏的三处分得很清：**改**在 {@link MaidActuator}，**读**全局那一层在
 * {@link PerceptionReader}、她身边那一层在 {@link SurroundingsReader}。判定一概不碰游戏
 * （{@link ActionRun}、{@link PerceptionDiff}、{@link SurroundingsDiff}），于是都能脱开
 * Minecraft 跑单测。
 *
 * <p>位移**不直接调 {@code getNavigation().moveTo()}**，而是每 tick 往 brain 里写
 * {@code WALK_TARGET} 记忆，由原版 {@code MoveToTargetSink} 自己驱动导航（车万女仆自己的
 * 找床、跟随也全是这么干的）。反过来做是自杀：清掉 WALK_TARGET 会让 sink 的 stop() 立刻
 * {@code navigation.stop()} 把路掐断。所以受理时只清一次 {@code PATH}（sink 要求它为 ABSENT
 * 才肯启动），之后每 tick 只写目标。
 *
 * <p>三条会让女仆「一动不动」的规矩，都在这里如实失败而不是空等：{@code MaidAwaitTask} 每
 * tick 清掉落在禁足半径之外的 WALK_TARGET（居家模式）；{@code MaidClearSleepTask} 每 tick
 * 把不在 REST 日程的睡觉女仆叫起来；玩家开着女仆 GUI 或跟随模式时，车万女仆自己的逻辑会
 * 抢走目标。
 */
public final class MaidActuator {
    /** 走过去的速度倍率：与车万女仆自己的跟随、去床边取同一个量级。 */
    private static final float WALK_SPEED = 0.5F;
    private static final float WALK_SPEED_TO_BED = 0.6F;
    /** 找床的半径：居家模式只在她自己的活动范围里找（超出去她也走不到），否则看周围这么远。 */
    private static final int BED_SEARCH_RADIUS = 32;
    /** 与车万女仆睡自己的女仆床时一致：躺进床的中心。 */
    private static final double SLEEP_OFFSET_Y = 0.8;

    /** 受理时就能判定的失败。 */
    public record Reject(String code, String reason) {
    }

    /** 动作的目标：{@code bedHead} 为 null 表示目标是女仆的主人。 */
    public record Target(BlockPos bedHead, Reject reject) {
        public boolean isBed() {
            return bedHead != null;
        }
    }

    private MaidActuator() {
    }

    // ─── 目标与预检 ───

    /** 解析目标：床要现找，玩家就是女仆的主人。找不到就给出拒绝原因。 */
    public static Target resolveTarget(EntityMaid maid, MaidAction action) {
        if (action.target() == MaidAction.Target.BED) {
            BedChoice.Pick pick = findBed(maid);
            if (pick.pos() == null) {
                return new Target(null, pick.sawOccupied()
                        ? new Reject("bed_occupied", "附近的床都被占了")
                        : new Reject("no_bed", "附近没有床"));
            }
            return new Target(new BlockPos(pick.pos().x(), pick.pos().y(), pick.pos().z()), null);
        }
        if (MaidQueries.owner(maid) == null) {
            return new Target(null, new Reject("player_away", "玩家不在她所在的这个世界里"));
        }
        return new Target(null, null);
    }

    /** 受理前的预检：这里能判定的都不留给超时，插件一次往返就知道行不行。 */
    public static Reject precheck(EntityMaid maid, MaidAction action, Target target) {
        if (!maid.isAlive()) {
            return new Reject("maid_gone", "她已经倒下了");
        }
        if (MaidQueries.carriedByPlayer(maid)) {
            return new Reject("maid_carried", "玩家正抱着她，先放下来");
        }
        if (maid.isPassenger()) {
            return new Reject("maid_riding", "她正骑着东西，动不了");
        }
        if (maid.isLeashed()) {
            return new Reject("maid_leashed", "她被拴住了，走不开");
        }
        // 站起是唯一会叫醒她的动作：别的动作在她睡着时做不了
        if (maid.isSleeping() && action.kind() != MaidAction.Kind.STAND) {
            return new Reject("maid_sleeping", "她正在睡觉，先让她站起来才叫得动");
        }
        if (action.walks() && maid.hasRestriction()) {
            BlockPos anchor = target.isBed() ? target.bedHead() : MaidQueries.owner(maid).blockPosition();
            if (!maid.isWithinRestriction(anchor)) {
                return new Reject("out_of_restriction", "她处于居家模式，目标在活动范围之外");
            }
        }
        return null;
    }

    // ─── 执行 ───

    /**
     * 站直：坐着就站起来、睡着就叫醒、坐在坐垫/椅子/船上就下来。返回有没有真的动过（回执里的 stood_up）。
     *
     * <p>坐垫、椅子、船这类「座位」上她只是一个乘客，坐姿标记并没有置上——只清坐姿她不会下车，
     * 而 {@code canBrainMoving()} 把乘客也算成动不了，所以站起来必须连车一起下。
     * 座位实体自己在没人坐之后会收拾掉（{@code EntitySit} 过十来个 tick 就 discard）。
     */
    public static boolean standUp(EntityMaid maid) {
        // 被玩家抱着时不能下车：那会拆散搬运数据，玩家看到的是「黏在手上」
        if (MaidQueries.carriedByPlayer(maid)) {
            return false;
        }
        boolean changed = false;
        if (maid.isSleeping()) {
            maid.stopSleeping();
            changed = true;
        }
        if (maid.isMaidInSittingPose()) {
            maid.setInSittingPose(false);
            changed = true;
        }
        if (maid.isPassenger()) {
            maid.stopRiding();
            changed = true;
        }
        return changed;
    }

    /** 坐下并回读复核：骑着东西时她坐不下去，这时得如实报失败。 */
    public static boolean sit(EntityMaid maid) {
        maid.setInSittingPose(true);
        return maid.isMaidInSittingPose();
    }

    /** 开始走：清一次残留的路径与「走不到」标记，否则旧的会挡住新的目标。 */
    public static void beginWalk(EntityMaid maid) {
        maid.getBrain().eraseMemory(MemoryModuleType.PATH);
        maid.getBrain().eraseMemory(MemoryModuleType.CANT_REACH_WALK_TARGET_SINCE);
    }

    /** 每 tick 写一次寻路目标。看向玩家的目标不带头部高度：走路用的是脚下那格。 */
    public static void walkToPlayer(EntityMaid maid, ServerPlayer player, int closeEnough) {
        maid.getBrain()
                .setMemory(
                        MemoryModuleType.WALK_TARGET,
                        new WalkTarget(new EntityTracker(player, false), WALK_SPEED, closeEnough));
    }

    /** 床走到床头那一格；与 sink 的判到位口径一致（曼哈顿 ≤ closeEnough）。 */
    public static void walkToBed(EntityMaid maid, BlockPos bedHead, int closeEnough) {
        maid.getBrain()
                .setMemory(
                        MemoryModuleType.WALK_TARGET,
                        new WalkTarget(Vec3.atBottomCenterOf(bedHead), WALK_SPEED_TO_BED, closeEnough));
    }

    /** 走向一只活物：攻击循环每 tick 写一次，closeEnough 给 1 格就够贴身。 */
    public static void walkToEntity(EntityMaid maid, LivingEntity target, int closeEnough) {
        maid.getBrain()
                .setMemory(
                        MemoryModuleType.WALK_TARGET,
                        new WalkTarget(new EntityTracker(target, false), WALK_SPEED_TO_BED, closeEnough));
    }

    /** 走向某一格：挖方块时走到事先挑好的落脚点。 */
    public static void walkToPos(EntityMaid maid, BlockPos pos, int closeEnough) {
        maid.getBrain()
                .setMemory(
                        MemoryModuleType.WALK_TARGET,
                        new WalkTarget(Vec3.atBottomCenterOf(pos), WALK_SPEED, closeEnough));
    }

    /** 转头盯着一只活物：攻击时每 tick 写一次，免得她背对着砍。 */
    public static void lookAtEntity(EntityMaid maid, LivingEntity target) {
        maid.getBrain().setMemory(MemoryModuleType.LOOK_TARGET, new EntityTracker(target, true));
        maid.getLookControl().setLookAt(target, 30.0F, 30.0F);
    }

    /** 转头看某一格：挖方块时让她真的朝着那块挖。 */
    public static void lookAtPos(EntityMaid maid, BlockPos pos) {
        maid.getBrain().setMemory(MemoryModuleType.LOOK_TARGET, new BlockPosTracker(pos));
        maid.getLookControl()
                .setLookAt(pos.getX() + 0.5, pos.getY() + 0.5, pos.getZ() + 0.5, 30.0F, 30.0F);
    }

    /** 转头看玩家：原生 LOOK_TARGET 加一层视线控制兜底。 */
    public static void lookAtPlayer(EntityMaid maid, ServerPlayer player) {
        maid.getBrain().setMemory(MemoryModuleType.LOOK_TARGET, new EntityTracker(player, true));
        maid.getLookControl().setLookAt(player, 30.0F, 30.0F);
    }

    /**
     * 说话时看向主人：每冒出一句气泡就调一次，不占动作槽、也不推任何帧。
     *
     * <p>打架、干活（非空闲任务）与睡觉时不打扰她——那几种状态下她的视线有自己的用场。
     * 只是转个头，所以不走 {@link MaidActionController}：那里是单槽位，顶掉正在走路的
     * 动作就本末倒置了。
     */
    public static void faceOwnerWhileTalking(EntityMaid maid) {
        if (maid.isSleeping() || maid.getTarget() != null) {
            return;
        }
        IMaidTask task = maid.getTask();
        if (task != null && !TaskIdle.UID.equals(task.getUid())) {
            return;
        }
        ServerPlayer player = MaidQueries.owner(maid);
        if (player == null || player.hasDisconnected() || player.level() != maid.level()) {
            return;
        }
        lookAtPlayer(maid, player);
    }

    /** 停下：清寻路目标与已算好的路径。 */
    public static void stopWalking(EntityMaid maid) {
        maid.getBrain().eraseMemory(MemoryModuleType.WALK_TARGET);
        maid.getBrain().eraseMemory(MemoryModuleType.PATH);
    }

    public static void stopLooking(EntityMaid maid) {
        maid.getBrain().eraseMemory(MemoryModuleType.LOOK_TARGET);
    }

    /**
     * 躺到床上。照车万女仆自己睡女仆床的做法：先停下来，再睡到床头那格的中心。
     *
     * <p>她的 {@code startSleeping} 被覆写过：回满血、涨好感——这是玩家想要的语义，
     * 所以这里不另写一套。
     */
    public static void sleepIn(EntityMaid maid, BlockPos bedHead) {
        stopWalking(maid);
        stopLooking(maid);
        maid.startSleeping(bedHead);
        maid.setPos(bedHead.getX() + 0.5, bedHead.getY() + SLEEP_OFFSET_Y, bedHead.getZ() + 0.5);
    }

    // ─── 观察 ───

    /** 把这一刻的游戏事实拍成状态机认识的样子。女仆不在了也照样给一份，让状态机收尾。 */
    public static ActionRun.Observation observe(EntityMaid maid, MaidAction action, Target target) {
        if (maid == null || !maid.isAlive()) {
            return new ActionRun.Observation(false, true, false, false, false, 0, 0);
        }
        boolean cannotReach = maid.getBrain().hasMemoryValue(MemoryModuleType.CANT_REACH_WALK_TARGET_SINCE);
        if (target.isBed()) {
            BlockPos head = target.bedHead();
            return new ActionRun.Observation(
                    true,
                    !maid.canBrainMoving(),
                    maid.isSleeping(),
                    isUsableBed(maid.level(), head),
                    cannotReach,
                    head.distManhattan(maid.blockPosition()),
                    Math.sqrt(maid.distanceToSqr(Vec3.atBottomCenterOf(head))));
        }
        ServerPlayer player = MaidQueries.owner(maid);
        if (player == null || player.hasDisconnected() || player.level() != maid.level()) {
            return new ActionRun.Observation(true, !maid.canBrainMoving(), maid.isSleeping(), false, cannotReach, 0, 0);
        }
        return new ActionRun.Observation(
                true,
                !maid.canBrainMoving(),
                maid.isSleeping(),
                true,
                cannotReach,
                player.blockPosition().distManhattan(maid.blockPosition()),
                Math.sqrt(maid.distanceToSqr(player)));
    }

    // ─── 找床 ───

    /** 最近的空床。索引里翻不到时给出「有没有床但被占了」这两个不同的理由。 */
    public static BedChoice.Pick findBed(EntityMaid maid) {
        if (!(maid.level() instanceof ServerLevel level)) {
            return new BedChoice.Pick(null, false);
        }
        BlockPos origin = maid.getBrainSearchPos();
        int radius = maid.hasRestriction() ? Math.max(8, (int) maid.getRestrictRadius()) : BED_SEARCH_RADIUS;
        List<BedChoice.Candidate> candidates = new ArrayList<>();
        level.getPoiManager()
                .getInRange(MaidActuator::isBedPoi, origin, radius, PoiManager.Occupancy.ANY)
                .forEach(record -> candidates.add(candidate(level, record.getPos())));
        BlockPos from = maid.blockPosition();
        return BedChoice.pick(candidates, from.getX(), from.getY(), from.getZ());
    }

    private static boolean isBedPoi(Holder<PoiType> holder) {
        return holder.unwrapKey()
                .map(key -> BedChoice.isBedPoiId(key.location().toString()))
                .orElse(false);
    }

    /**
     * 把 POI 索引里的一格复核成方块事实。
     *
     * <p>两套床走两套属性：原版床（含别的模组的 BedBlock 子类）用 {@link BedBlock} 的
     * PART / OCCUPIED，车万女仆的女仆床是自己的方块（继承的是水平朝向方块），用
     * {@link BlockMaidBed} 的那一对。索引只当线索，是不是床以方块为准。
     */
    private static BedChoice.Candidate candidate(Level level, BlockPos pos) {
        BlockState state = level.getBlockState(pos);
        if (state.getBlock() instanceof BedBlock) {
            return new BedChoice.Candidate(
                    pos.getX(),
                    pos.getY(),
                    pos.getZ(),
                    true,
                    state.getValue(BedBlock.PART) == BedPart.HEAD,
                    state.getValue(BedBlock.OCCUPIED));
        }
        if (state.is(InitBlocks.MAID_BED.get())) {
            return new BedChoice.Candidate(
                    pos.getX(),
                    pos.getY(),
                    pos.getZ(),
                    true,
                    state.getValue(BlockMaidBed.PART) == BedPart.HEAD,
                    state.getValue(BlockMaidBed.OCCUPIED));
        }
        return new BedChoice.Candidate(pos.getX(), pos.getY(), pos.getZ(), false, false, false);
    }

    /** 走到一半床被挖了、被占了，也要当成「目标不见了」。 */
    private static boolean isUsableBed(Level level, BlockPos pos) {
        BedChoice.Candidate candidate = candidate(level, pos);
        return candidate.bedBlock() && candidate.head() && !candidate.occupied();
    }
}
