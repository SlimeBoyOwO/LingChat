package com.lingchat.maidbridge.action.core;

import com.github.tartaricacid.touhoulittlemaid.api.task.IMaidTask;
import com.github.tartaricacid.touhoulittlemaid.block.BlockMaidBed;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.github.tartaricacid.touhoulittlemaid.entity.task.TaskIdle;
import com.github.tartaricacid.touhoulittlemaid.init.InitBlocks;
import com.lingchat.maidbridge.maid.MaidCondition;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.world.PerceptionDiff;
import com.lingchat.maidbridge.perception.world.PerceptionReader;
import com.lingchat.maidbridge.perception.nearby.SurroundingsDiff;
import com.lingchat.maidbridge.perception.nearby.SurroundingsReader;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.Holder;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.Pose;
import net.minecraft.world.entity.ai.behavior.BlockPosTracker;
import net.minecraft.world.entity.ai.behavior.EntityTracker;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.ai.memory.WalkTarget;
import net.minecraft.world.entity.ai.village.poi.PoiManager;
import net.minecraft.world.entity.ai.village.poi.PoiType;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.BedBlock;
import net.minecraft.world.level.block.HorizontalDirectionalBlock;
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
    /** 在床上躺着休息的回血节奏：每 10 秒一滴（tick，20 tick = 1 秒）。 */
    private static final int REST_HEAL_TICKS = 200;
    /** 探路时她停在主人正前方多远（格）。 */
    private static final int SCOUT_DISTANCE = 8;
    /** 探路点新算出来的差这么几格以内就不换：主人挪一小步、转个身时别让她原地打转。 */
    private static final int SCOUT_DRIFT = 2;
    /** 探路时离主人超过这么远就算走散了：她跟不上，再追也没意义。 */
    private static final float SCOUT_LEASH = 24.0F;

    /** 床上休息的回血计时：只剩一个动作槽，记一个就够。 */
    private static String restMaid = "";
    private static int restTicks;

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
        // 就地躺下不要目标：既不找床，也不找人
        if (action.isGroundLie()) {
            return new Target(null, null);
        }
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
        if (MaidCondition.isUnconscious(maid)) {
            return new Reject("maid_unconscious", "她昏迷不醒，叫不动");
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
        // 站起是唯一会叫醒她的动作：别的动作在她睡着时做不了。
        // 桥接自己按着的躺姿不算：那时玩家喊一声「过来」就该把她叫起来带走
        if (maid.isSleeping() && action.kind() != MaidAction.Kind.STAND
                && !MaidActionController.isHoldingLie(maid)) {
            return new Reject("maid_sleeping", "她正在睡觉，先让她站起来才叫得动");
        }
        if (action.walks() && maid.hasRestriction()) {
            BlockPos anchor = target.isBed() ? target.bedHead() : MaidQueries.owner(maid).blockPosition();
            if (!maid.isWithinRestriction(anchor)) {
                return new Reject("out_of_restriction", "她处于居家模式，目标在活动范围之外");
            }
        }
        if (action.isGroundLie()) {
            // 原地躺下：脚下得有地，泡在液体里会被原版强制切成游泳姿
            if (maid.isInWater() || maid.isInLava()) {
                return new Reject("maid_in_liquid", "她泡在液体里，躺不下去");
            }
            BlockPos below = maid.blockPosition().below();
            if (!maid.level().getBlockState(below).isFaceSturdy(maid.level(), below, Direction.UP)) {
                return new Reject("no_ground", "她脚下没有实地，躺不下去");
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
        } else if (maid.hasPose(Pose.SLEEPING)) {
            wakeUp(maid);
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

    /**
     * 探路要去的那一格：主人面朝的方向前面约 8 格、站得下她的地方。
     *
     * <p>沿朝向逐格往回试，先远后近：前面的路抬高一层、往下掉一层都认，脚下是水或岩浆
     * 的不算落脚点。一路都是墙时退到最近能站的那一格；连一格都站不下就给 null，由调用方
     * 退回「跟住主人」。居家模式下只认活动范围里的格子，否则车万女仆的等待行为会当场把
     * 写进去的寻路目标清掉。
     *
     * @param previous 上一次的落点：新算出来的没挪出 {@link #SCOUT_DRIFT} 格就接着用它，
     *     免得主人站着晃视角时她来回走
     */
    public static BlockPos scoutPoint(EntityMaid maid, ServerPlayer player, BlockPos previous) {
        BlockPos fresh = scoutCandidate(maid, player);
        if (previous != null && (fresh == null || fresh.distManhattan(previous) <= SCOUT_DRIFT)) {
            return previous;
        }
        return fresh;
    }

    private static BlockPos scoutCandidate(EntityMaid maid, ServerPlayer player) {
        Level level = player.level();
        double yaw = Math.toRadians(player.getYRot());
        double dx = -Math.sin(yaw);
        double dz = Math.cos(yaw);
        BlockPos base = player.blockPosition();
        for (int step = SCOUT_DISTANCE; step >= 2; step--) {
            int x = (int) Math.floor(base.getX() + 0.5 + dx * step);
            int z = (int) Math.floor(base.getZ() + 0.5 + dz * step);
            for (int dy = 1; dy >= -3; dy--) {
                BlockPos feet = new BlockPos(x, base.getY() + dy, z);
                if (maid.isWithinRestriction(feet) && standingRoom(level, feet)) {
                    return feet;
                }
            }
        }
        return null;
    }

    /** 那一格能不能站人：脚下有实地，脚与头这两格都是空的。 */
    private static boolean standingRoom(Level level, BlockPos feet) {
        BlockPos below = feet.below();
        if (!level.getBlockState(below).isFaceSturdy(level, below, Direction.UP)) {
            return false;
        }
        return level.getBlockState(feet).getCollisionShape(level, feet).isEmpty()
                && level.getBlockState(feet.above()).getCollisionShape(level, feet.above()).isEmpty();
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
     * 就地躺下：换姿势、记一个脚下的睡位，不碰床、也不占用任何床位。
     *
     * <p>睡位不是为了真睡，而是给「她躺着」一个所有模组都认得的说法：YSM 的闭眼与躺姿、
     * 车万女仆的别乱走都只看 {@code isSleeping()}。原版「睡位不是床就叫起来」与车万女仆
     * 的日程清理由 {@code LivingEntityStopSleepingMixin} 在这个动作期间拦掉。
     */
    public static void lieOnGround(EntityMaid maid) {
        stopWalking(maid);
        stopLooking(maid);
        maid.setDeltaMovement(Vec3.ZERO);
        maid.setPose(Pose.SLEEPING);
        maid.setSleepingPos(maid.blockPosition());
        maid.refreshDimensions();
        MaidCondition.holdGroundLift(maid);
    }

    /**
     * 躺着时每 tick 按住：清掉别人写进来的寻路目标，别让她带着躺姿滑出去。
     *
     * <p>睡位与姿势一起按：YSM 的闭眼与躺姿、车万女仆的「别乱走」看的都是
     * {@code isSleeping()}，光有姿势没有睡位在游戏里等于没躺下。床上的睡位被人清掉
     * （原版「床没了」、别的模组叫醒）就当场补回去；人不在床头那一格就摆回床头中心，
     * 剩下的床位吸附交给车万女仆自己的 {@code onMaidSleep}。
     */
    public static void holdLying(EntityMaid maid, BlockPos bedHead) {
        stopWalking(maid);
        maid.setDeltaMovement(Vec3.ZERO);
        if (!maid.hasPose(Pose.SLEEPING)) {
            maid.setPose(Pose.SLEEPING);
        }
        if (bedHead != null) {
            if (!bedHead.equals(maid.getSleepingPos().orElse(null))) {
                maid.setSleepingPos(bedHead);
            }
            // 人不在床头那一格就摆到床头中心：她可能迈步时已经站上了床沿，也可能正踩着床尾。
            // 只按「脚下有没有床」判，这两种会被当成已经躺好，随后那层的吸附又被拦掉，
            // 她就一直躺在床尾——头离开枕头、身子往床外错出去一格。
            if (!bedHead.equals(maid.blockPosition())) {
                maid.setPos(bedHead.getX() + 0.5, bedHead.getY() + SLEEP_OFFSET_Y, bedHead.getZ() + 0.5);
            }
            restOnBed(maid, bedHead);
            return;
        }
        MaidCondition.holdGroundLift(maid);
        if (!maid.blockPosition().equals(maid.getSleepingPos().orElse(null))) {
            maid.setSleepingPos(maid.blockPosition());
        }
    }

    /**
     * 躺在床上时慢慢回血：每 10 秒一滴。
     *
     * <p>只有人真的在床头那一格里才算——刚被摆上去的那一 tick 也算，反正位置已经改过了。
     * 起身、换动作、满血都从这里走开，计时清零，下次躺下从头算。
     */
    private static void restOnBed(EntityMaid maid, BlockPos bedHead) {
        String maidId = maid.getStringUUID();
        if (!maidId.equals(restMaid)) {
            restMaid = maidId;
            restTicks = 0;
        }
        if (!bedHead.equals(maid.blockPosition()) || maid.getHealth() >= maid.getMaxHealth()) {
            restTicks = 0;
            return;
        }
        if (++restTicks < REST_HEAL_TICKS) {
            return;
        }
        restTicks = 0;
        maid.heal(1.0F);
    }

    /**
     * 从躺姿里起来：床上走原版起身，地上把位置放回原来那一格。
     *
     * <p>躺姿的碰撞箱只有 0.2 格高，换姿势时原版会「保持盒子中心」把人往下挪半截；所以先
     * 记下原坐标或先落回地面，站直后再放回去——原版 {@code stopSleeping} 也是这么写的。
     *
     * <p>被搬运模组抱在手里时也把睡姿收掉，否则公主抱会套着躺姿；位置交给搬运，不在这里动。
     */
    public static void wakeUp(EntityMaid maid) {
        if (maid.isSleeping() && MaidCondition.isSleepingOnBed(maid)) {
            // 真在床上的交给原版：它会把她挪到床边站好，并放开床位
            maid.stopSleeping();
            return;
        }
        if (!maid.isSleeping() && !maid.hasPose(Pose.SLEEPING)) {
            return;
        }
        MaidCondition.settleGroundLift(maid);
        Vec3 pos = maid.position();
        // 睡位在脚下那一格：原版这条路不认床，起身与清睡位都自己来
        maid.stopSleeping();
        maid.setPose(Pose.STANDING);
        maid.setPos(pos.x, pos.y, pos.z);
        maid.refreshDimensions();
    }

    // ─── 观察 ───

    /**
     * 把这一刻的游戏事实拍成状态机认识的样子。女仆不在了也照样给一份，让状态机收尾。
     *
     * @param keepLying 状态机已经在躺着了：这时才去查「什么让她躺不住了」，
     *     还在走路的阶段把这些当成打断会让一次正常赶路半途而废
     */
    public static ActionRun.Observation observe(
            EntityMaid maid, MaidAction action, Target target, boolean keepLying) {
        if (maid == null || !maid.isAlive()) {
            return new ActionRun.Observation(
                    false, true, false, false, false, 0, 0, ActionRun.Interrupt.NONE);
        }
        if (action.isGroundLie()) {
            return new ActionRun.Observation(
                    true, false, maid.isSleeping(), true, false, 0, 0.0,
                    keepLying ? groundInterrupt(maid) : ActionRun.Interrupt.NONE);
        }
        if (action.isScout()) {
            ServerPlayer owner = MaidQueries.owner(maid);
            return new ActionRun.Observation(
                    true,
                    !maid.canBrainMoving(),
                    maid.isSleeping(),
                    true,
                    false,
                    0,
                    owner == null ? 0.0 : Math.sqrt(maid.distanceToSqr(owner)),
                    scoutInterrupt(maid, owner));
        }
        boolean cannotReach = maid.getBrain().hasMemoryValue(MemoryModuleType.CANT_REACH_WALK_TARGET_SINCE);
        if (target.isBed()) {
            BlockPos head = target.bedHead();
            // 已经躺在这张床上了：这时只关心床还在不在，床位被登记成占用不算目标不见了
            boolean present = isUsableBed(maid.level(), head) || (keepLying && isBedHead(maid.level(), head));
            return new ActionRun.Observation(
                    true,
                    !maid.canBrainMoving(),
                    maid.isSleeping(),
                    present,
                    cannotReach,
                    head.distManhattan(maid.blockPosition()),
                    Math.sqrt(maid.distanceToSqr(Vec3.atBottomCenterOf(head))),
                    keepLying ? bedInterrupt(maid, head) : ActionRun.Interrupt.NONE);
        }
        ServerPlayer player = MaidQueries.owner(maid);
        if (player == null || player.hasDisconnected() || player.level() != maid.level()) {
            return new ActionRun.Observation(
                    true, !maid.canBrainMoving(), maid.isSleeping(), false, cannotReach, 0, 0,
                    ActionRun.Interrupt.NONE);
        }
        return new ActionRun.Observation(
                true,
                !maid.canBrainMoving(),
                maid.isSleeping(),
                true,
                cannotReach,
                player.blockPosition().distManhattan(maid.blockPosition()),
                Math.sqrt(maid.distanceToSqr(player)),
                ActionRun.Interrupt.NONE);
    }

    /**
     * 地上躺不住的那几件事：被抱走、挨打、泡进液体、身下没地。
     *
     * <p>只有她真的躺不住时才起来；玩家站在旁边、天黑下雨都不算。睡位是我们自己按在
     * 脚下的，不能拿 {@code isSleeping()} 判断「她爬上床了」。
     */
    private static ActionRun.Interrupt groundInterrupt(EntityMaid maid) {
        if (MaidQueries.carriedByPlayer(maid)) {
            return new ActionRun.Interrupt("carried", "被抱起来了");
        }
        if (maid.hurtTime > 0) {
            return new ActionRun.Interrupt("hurt", "挨了一下");
        }
        if (maid.isInWater() || maid.isInLava()) {
            return new ActionRun.Interrupt("in_liquid", "泡进了液体里");
        }
        if (!maid.onGround() && !MaidCondition.isGroundLifted(maid)) {
            return new ActionRun.Interrupt("lost_ground", "身下没有实地了");
        }
        return ActionRun.Interrupt.NONE;
    }

    /**
     * 探路走不下去的那几件事：血不多了、被抱走、动不了、主人走了或走散了。
     *
     * <p>血线按最大血量算，不到四分之一就让她退下来——挡在前面的是她，血量见底时
     * 再往前送就是白白挨刀。
     */
    private static ActionRun.Interrupt scoutInterrupt(EntityMaid maid, ServerPlayer player) {
        if (MaidQueries.carriedByPlayer(maid)) {
            return new ActionRun.Interrupt("carried", "被抱起来了");
        }
        if (maid.getMaxHealth() > 0 && maid.getHealth() < maid.getMaxHealth() * 0.25F) {
            return new ActionRun.Interrupt("low_health", "血不多了");
        }
        if (!maid.canBrainMoving()) {
            return new ActionRun.Interrupt("maid_busy", "她现在动不了");
        }
        if (player == null || player.hasDisconnected() || player.level() != maid.level()) {
            return new ActionRun.Interrupt("player_away", "主人不在这个世界里了");
        }
        if (maid.distanceTo(player) > SCOUT_LEASH) {
            return new ActionRun.Interrupt("too_far", "跟主人走散了");
        }
        return ActionRun.Interrupt.NONE;
    }

    /** 床上躺不住的那几件事：被抱走、挨打、床没了、睡位被人清掉。 */
    private static ActionRun.Interrupt bedInterrupt(EntityMaid maid, BlockPos head) {
        if (MaidQueries.carriedByPlayer(maid)) {
            return new ActionRun.Interrupt("carried", "被抱起来了");
        }
        if (maid.hurtTime > 0) {
            return new ActionRun.Interrupt("hurt", "挨了一下");
        }
        if (!isBedHead(maid.level(), head)) {
            return new ActionRun.Interrupt("bed_gone", "床没了");
        }
        if (!maid.isSleeping()) {
            return new ActionRun.Interrupt("woken", "自己爬了起来");
        }
        return ActionRun.Interrupt.NONE;
    }

    // ─── 找床 ───

    /**
     * 这一格床的床头在哪一格：拿到的格子是床尾时顺着朝向翻过去；不是床就给 null。
     *
     * <p>睡位必须压在床头：车万女仆每 tick 把实体摆到睡位那一格的中心，渲染又按睡位那格
     * 床的朝向把身子往床尾铺开一格半。睡位落在床尾，她整个人就往床尾错出去一格——头离开
     * 枕头、身子压在床的下半截。两套床的 FACING 都是「床尾指向床头」。
     */
    public static BlockPos bedHeadOf(Level level, BlockPos pos) {
        BlockState state = level.getBlockState(pos);
        BedPart part;
        if (state.getBlock() instanceof BedBlock) {
            part = state.getValue(BedBlock.PART);
        } else if (state.is(InitBlocks.MAID_BED.get())) {
            part = state.getValue(BlockMaidBed.PART);
        } else {
            return null;
        }
        if (part == BedPart.HEAD) {
            return pos;
        }
        return pos.relative(state.getValue(HorizontalDirectionalBlock.FACING));
    }

    /** 她脚下这张床的床头（脚下或脚下那一格是床都认）；不在床上给 null。 */
    public static BlockPos bedHeadAt(EntityMaid maid) {
        if (maid == null) {
            return null;
        }
        BlockPos head = bedHeadOf(maid.level(), maid.blockPosition());
        return head != null ? head : bedHeadOf(maid.level(), maid.blockPosition().below());
    }

    /**
     * 最近的空床。索引里翻不到时给出「有没有床但被占了」这两个不同的理由。
     */
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

    /** 那一格还是床头：躺着的时候只关心床还在不在，占用是自己占的。 */
    private static boolean isBedHead(Level level, BlockPos pos) {
        BedChoice.Candidate candidate = candidate(level, pos);
        return candidate.bedBlock() && candidate.head();
    }
}
