package com.lingchat.maidbridge.maid;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.action.attack.MaidAttackController;
import com.lingchat.maidbridge.action.core.MaidActionController;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.action.mine.MaidMineController;
import com.lingchat.maidbridge.action.place.MaidPlaceController;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.perception.EventFrames;
import com.lingchat.maidbridge.perception.PerceptionSampler;
import com.lingchat.maidbridge.perception.event.Attackers;
import com.lingchat.maidbridge.perception.event.HurtReporter;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.particles.ParticleTypes;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.damagesource.DamageTypes;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.Pose;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.Vec3;
import net.neoforged.neoforge.event.entity.living.LivingDamageEvent;
import net.neoforged.neoforge.event.entity.living.LivingDeathEvent;
import net.neoforged.neoforge.event.entity.living.LivingIncomingDamageEvent;

/**
 * 她的昏迷状态：致命伤只把她打晕，不让她死。
 *
 * <p>原版 0 血会进入死亡计时并移除实体，所以致命伤结算后、死亡判定前把血拉回 1 点。
 * 昏迷期间普通伤害全部取消；只有 {@code /kill}（generic_kill）与掉虚空（out_of_world）
 * 仍然走旧死亡流程。
 *
 * <p>闭眼与睡眠动画靠的是睡眠位置：车万女仆与 YSM 的 {@code is_close_eyes} 都只看
 * {@code isSleeping()}。这里直接写睡眠位置与睡姿，不调用车万女仆的 {@code startSleeping}
 * （它会回满血、涨好感）；原版与车万女仆每 tick 的叫醒由 {@code stopSleeping} 拦截，
 * 非床的睡眠位置还要跳过 {@code onMaidSleep} 把位置吸到床中心的那一下。被搬运模组抱着时
 * 姿势改回站立，睡位只负责闭眼，好让 YSM 的公主抱动画正常播放；地上睡姿抬离方块 0.2 格，
 * 避免模型陷进地里。
 *
 * <p>状态与恢复进度都写在实体的持久化 NBT 里，区块卸载、重进、重启都不丢；被抱在手里
 * 或区块未加载时不计时。普通伤害在昏迷期间被取消，只有上面那两种致命来源能真正杀死她。
 */
public final class MaidCondition {
    /** 床上连续躺这么久才醒（tick，20 tick = 1 秒）。 */
    public static final int RECOVERY_TICKS = 1200;
    /** 恢复期间冒爱心的间隔。 */
    private static final int HEART_INTERVAL = 10;
    /** 未加载区块里的昏迷女仆靠这个周期重新认回来。 */
    private static final int DISCOVER_INTERVAL = 20;
    private static final String TAG_UNCONSCIOUS = "LingChatUnconscious";
    private static final String TAG_RECOVERY = "LingChatRecovery";
    /** 地上躺姿要抬离地面一点：YSM 的睡姿模型按脚底对齐，不抬会陷进方块里。 */
    private static final double GROUND_LIFT = 0.2;
    private static final String TAG_LIFTED = "LingChatLifted";
    private static final String TAG_GROUND_Y = "LingChatGroundY";

    /** 服务端记着哪些女仆正在昏迷；持久化标记仍然是 NBT。 */
    private static final Set<String> ACTIVE = new HashSet<>();
    private static int discoverClock;

    private MaidCondition() {
    }

    public static boolean isUnconscious(EntityMaid maid) {
        return maid != null && maid.getPersistentData().getBoolean(TAG_UNCONSCIOUS);
    }

    /** 客户端也知道她昏着：碰撞箱不能只按服务端的持久化数据算。 */
    public static boolean isUnconsciousSynced(EntityMaid maid) {
        return MaidUnconsciousData.is(maid);
    }

    private static void markSynced(EntityMaid maid, boolean value) {
        MaidUnconsciousData.set(maid, value);
    }

    /** 实体没加载时（被抱走、区块卸载）也能靠服务端记着的 uuid 回答。 */
    public static boolean isUnconscious(String maidId) {
        return maidId != null && ACTIVE.contains(maidId);
    }

    /** 她是不是正躺在可恢复的床上：被抱着、坐在载具里都不算。 */
    public static boolean isOnBed(EntityMaid maid) {
        return maid != null
                && !maid.isPassenger()
                && !MaidQueries.carriedByPlayer(maid)
                && bedAt(maid) != null;
    }

    /** 脚下或脚下那一格的床；普通床与女仆床都认。 */
    public static BlockPos bedAt(EntityMaid maid) {
        if (maid == null) {
            return null;
        }
        Level level = maid.level();
        BlockPos pos = maid.blockPosition();
        if (isBed(level, pos, maid)) {
            return pos;
        }
        BlockPos below = pos.below();
        return isBed(level, below, maid) ? below : null;
    }

    private static boolean isBed(Level level, BlockPos pos, EntityMaid maid) {
        BlockState state = level.getBlockState(pos);
        return state.isBed(level, pos, maid);
    }

    /** 她的睡位指着的是一张床：起身要走原版流程，还是我们自己把睡位按在脚下。 */
    public static boolean isSleepingOnBed(EntityMaid maid) {
        if (maid == null) {
            return false;
        }
        BlockPos pos = maid.getSleepingPos().orElse(null);
        return pos != null && maid.level().getBlockState(pos).isBed(maid.level(), pos, maid);
    }

    /**
     * 这一 tick 不要按旧睡位吸附。
     *
     * <p>车万女仆的 {@code onMaidSleep} 会把位置吸到睡位中心：睡在地上时会浮起来，
     * 被抱着时会和搬运抢位置。她被刚放上床时睡位还停在旧地方，也要先跳过一 tick，
     * 等 {@link #tick} 把睡位改成床，否则 {@code onMaidSleep} 会把她拽回旧位置。
     *
     * <p>按床头比而不是按脚下那一格：她可能正踩在床尾那半张，床头对得上就该让它吸附，
     * 那一吸正好把人对齐到床头中心。
     */
    public static boolean isSleepingOffBed(EntityMaid maid) {
        if (maid == null || !maid.isSleeping()) {
            return false;
        }
        if (maid.isPassenger()) {
            return true;
        }
        BlockPos bed = MaidActuator.bedHeadAt(maid);
        if (bed == null) {
            return true;
        }
        return !bed.equals(maid.getSleepingPos().orElse(null));
    }

    /** 床上恢复还剩多少秒；不在床上时为 0。 */
    public static int recoverySecondsLeft(EntityMaid maid) {
        if (maid == null || !isUnconscious(maid) || !isOnBed(maid)) {
            return 0;
        }
        int done = maid.getPersistentData().getInt(TAG_RECOVERY);
        return Math.max(0, (RECOVERY_TICKS - done + 19) / 20);
    }

    /** 她现在是不是被抬离地面躺着。 */
    public static boolean isGroundLifted(EntityMaid maid) {
        return maid != null && maid.getPersistentData().getBoolean(TAG_LIFTED);
    }

    /**
     * 地上躺下时抬离地面一点。
     *
     * <p>YSM 的睡姿模型按脚底对齐，直接躺在方块表面会陷进地里；床上的高度由车万女仆
     * 自己的睡位吸附负责，所以只在地面用。抬起来后关掉重力，脚下那块方块没了就放下去
     * 让她正常落地。
     */
    public static void holdGroundLift(EntityMaid maid) {
        if (maid == null || maid.isPassenger()) {
            clearGroundLift(maid);
            return;
        }
        CompoundTag tag = maid.getPersistentData();
        if (!tag.getBoolean(TAG_LIFTED)) {
            if (!maid.onGround()) {
                return;
            }
            double groundY = maid.getY();
            tag.putBoolean(TAG_LIFTED, true);
            tag.putDouble(TAG_GROUND_Y, groundY);
            maid.setNoGravity(true);
            maid.setPos(maid.getX(), groundY + GROUND_LIFT, maid.getZ());
            maid.setDeltaMovement(Vec3.ZERO);
            return;
        }
        double groundY = tag.getDouble(TAG_GROUND_Y);
        BlockPos support = BlockPos.containing(maid.getX(), groundY - 0.05, maid.getZ());
        if (maid.level().getBlockState(support).getCollisionShape(maid.level(), support).isEmpty()) {
            clearGroundLift(maid);
            return;
        }
        if (!maid.isNoGravity()) {
            maid.setNoGravity(true);
        }
        maid.setPos(maid.getX(), groundY + GROUND_LIFT, maid.getZ());
        maid.setDeltaMovement(Vec3.ZERO);
    }

    /** 关掉抬升，位置先不动（上床、被抱、死亡时各有自己的落点）。 */
    public static void clearGroundLift(EntityMaid maid) {
        if (maid == null) {
            return;
        }
        CompoundTag tag = maid.getPersistentData();
        if (!tag.getBoolean(TAG_LIFTED)) {
            return;
        }
        tag.putBoolean(TAG_LIFTED, false);
        tag.remove(TAG_GROUND_Y);
        maid.setNoGravity(false);
    }

    /** 关掉抬升并落回原来的地面高度：站起来用；被抱着时只关掉，位置交给搬运。 */
    public static void settleGroundLift(EntityMaid maid) {
        if (maid == null) {
            return;
        }
        CompoundTag tag = maid.getPersistentData();
        if (!tag.getBoolean(TAG_LIFTED)) {
            return;
        }
        if (maid.isPassenger()) {
            clearGroundLift(maid);
            return;
        }
        double groundY = tag.getDouble(TAG_GROUND_Y);
        clearGroundLift(maid);
        maid.setPos(maid.getX(), groundY, maid.getZ());
    }

    /** 每服务端 tick 推进一次：发现昏迷女仆、床上计时、冒爱心、到点醒来。 */
    public static void tick(MinecraftServer server) {
        if (++discoverClock >= DISCOVER_INTERVAL) {
            discoverClock = 0;
            discover(server);
        }
        if (ACTIVE.isEmpty()) {
            return;
        }
        for (String id : new ArrayList<>(ACTIVE)) {
            EntityMaid maid = MaidQueries.findMaidById(server, id);
            if (maid == null) {
                // 被 Carry On 抱走或区块卸载：先挂着，回来再继续
                continue;
            }
            if (!maid.isAlive() || maid.isRemoved()) {
                clear(maid);
                continue;
            }
            if (!isUnconscious(maid)) {
                ACTIVE.remove(id);
                settleGroundLift(maid);
                continue;
            }
            tickOne(maid);
        }
    }

    private static void discover(MinecraftServer server) {
        for (EntityMaid maid : MaidQueries.getAllMaids(server)) {
            markSynced(maid, isUnconscious(maid));
            if (isUnconscious(maid)) {
                ACTIVE.add(maid.getStringUUID());
            } else {
                ACTIVE.remove(maid.getStringUUID());
                // 地上躺下的动作状态不跨存档：留下的抬升标记要收掉，别让她悬着
                if (!MaidActionController.isGroundLying(maid)) {
                    settleGroundLift(maid);
                }
            }
        }
    }

    private static void tickOne(EntityMaid maid) {
        CompoundTag tag = maid.getPersistentData();
        if (maid.getHealth() <= 0.0F) {
            // 绕过伤害直接设 0 的死亡仍走旧路
            clear(maid);
            return;
        }
        if (maid.getHealth() > 1.0F) {
            maid.setHealth(1.0F);
        }
        // 睡位一直留着：YSM 的 is_close_eyes 看的是 isSleeping，不是姿势
        if (!maid.isSleeping()) {
            maid.setSleepingPos(maid.blockPosition());
        }
        if (maid.isSwimming()) {
            maid.setSwimming(false);
        }
        if (maid.isMaidInSittingPose()) {
            maid.setInSittingPose(false);
        }
        if (!maid.isSilent()) {
            maid.setSilent(true);
        }

        // 被搬运动模组抱着：姿势交还给搬运与 YSM 的公主抱，只保留闭眼
        if (maid.isPassenger()) {
            clearGroundLift(maid);
            if (!maid.hasPose(Pose.STANDING)) {
                maid.setPose(Pose.STANDING);
            }
            MaidActuator.stopLooking(maid);
            maid.setYHeadRot(maid.yBodyRot);
            if (tag.getInt(TAG_RECOVERY) != 0) {
                tag.putInt(TAG_RECOVERY, 0);
            }
            return;
        }

        // 车万女仆的工作/跟随仍可能往 brain 里写目标；昏迷时每 tick 清掉
        MaidActuator.stopWalking(maid);
        MaidActuator.stopLooking(maid);
        maid.setYHeadRot(maid.yBodyRot);

        BlockPos bed = isOnBed(maid) ? MaidActuator.bedHeadAt(maid) : null;
        if (bed != null) {
            clearGroundLift(maid);
            if (!maid.hasPose(Pose.SLEEPING)) {
                maid.setPose(Pose.SLEEPING);
            }
            if (!bed.equals(maid.getSleepingPos().orElse(null))) {
                maid.setSleepingPos(bed);
            }
            int recovery = tag.getInt(TAG_RECOVERY);
            if (recovery == 0) {
                broadcastRecovering(maid);
            }
            recovery++;
            tag.putInt(TAG_RECOVERY, recovery);
            if (recovery % HEART_INTERVAL == 0) {
                heart(maid);
            }
            if (recovery >= RECOVERY_TICKS) {
                revive(maid);
            }
            return;
        }

        if (tag.getInt(TAG_RECOVERY) != 0) {
            tag.putInt(TAG_RECOVERY, 0);
        }
        // 离开床后把睡位换回脚下的格子，避免 onMaidSleep 或原版床检查把她按回床上
        BlockPos current = maid.blockPosition();
        if (!current.equals(maid.getSleepingPos().orElse(null))) {
            maid.setSleepingPos(current);
        }
        if (!maid.hasPose(Pose.SLEEPING)) {
            maid.setPose(Pose.SLEEPING);
        }
        holdGroundLift(maid);
    }

    /** 致命伤结算后、原版死亡判定前调用。 */
    public static void enter(EntityMaid maid, DamageSource source) {
        if (maid == null || maid.level().isClientSide() || isUnconscious(maid)) {
            return;
        }
        if (!maid.getStringUUID().equals(MonitoredMaidHandler.get())) {
            return;
        }
        MinecraftServer server = maid.getServer();
        if (server != null) {
            MaidActionController.cancelForUnconscious(server);
            MaidAttackController.cancel();
            MaidMineController.cancelForUnconscious(server);
            MaidPlaceController.cancelForUnconscious(server);
        }
        // 先把坐姿、载具、普通睡眠都收掉；被玩家抱着时 standUp 会自己避开
        MaidActuator.standUp(maid);
        maid.setSwimming(false);

        CompoundTag tag = maid.getPersistentData();
        tag.putBoolean(TAG_UNCONSCIOUS, true);
        tag.putInt(TAG_RECOVERY, 0);
        markSynced(maid, true);
        ACTIVE.add(maid.getStringUUID());

        // 留 1 点血：0 血会进原版死亡计时
        maid.setHealth(1.0F);
        // 倒在床上就按这张床的床头：压着床尾那半张摆睡位，人会往床外错一格
        BlockPos bed = MaidActuator.bedHeadAt(maid);
        if (maid.isPassenger()) {
            // 搬运模组抱着她时不能压成睡姿，否则公主抱动画会被睡姿顶掉；
            // 睡位留着只为闭眼。
            clearGroundLift(maid);
            maid.setPose(Pose.STANDING);
            maid.setSleepingPos(maid.blockPosition());
            maid.setYHeadRot(maid.yBodyRot);
        } else {
            maid.setPose(Pose.SLEEPING);
            maid.setSleepingPos(bed != null ? bed : maid.blockPosition());
            maid.setDeltaMovement(Vec3.ZERO);
            if (bed == null) {
                holdGroundLift(maid);
            } else {
                clearGroundLift(maid);
            }
        }
        // 已经在睡姿时 setPose 不会重算碰撞箱，这里强制刷一次
        maid.refreshDimensions();
        maid.setSilent(true);
        maid.getChatBubbleManager().getChatBubbleDataCollection().chatBubbles().clear();
        maid.clearFire();
        maid.setSharedFlagOnFire(false);
        HurtReporter.clear();
        PerceptionSampler.onUnconscious();
        boolean onBed = bed != null && !maid.isPassenger() && !MaidQueries.carriedByPlayer(maid);
        broadcastUnconscious(maid, source, onBed);
    }

    /** 床上躺满一分钟：清掉昏迷，醒来只有一点血。 */
    public static void revive(EntityMaid maid) {
        if (maid == null) {
            return;
        }
        CompoundTag tag = maid.getPersistentData();
        tag.putBoolean(TAG_UNCONSCIOUS, false);
        tag.putInt(TAG_RECOVERY, 0);
        markSynced(maid, false);
        ACTIVE.remove(maid.getStringUUID());

        maid.setSilent(false);
        maid.stopSleeping();
        maid.setPose(Pose.STANDING);
        settleGroundLift(maid);
        maid.setDeltaMovement(Vec3.ZERO);
        maid.setHealth(1.0F);
        maid.refreshDimensions();
        MaidActuator.stopWalking(maid);
        MaidActuator.stopLooking(maid);
        heartsAround(maid, 8);
        broadcastRecovered(maid);
    }

    /** 真死亡（/kill、虚空、直接 die）走旧路：清掉昏迷标记，别让 stopSleeping 拦着。 */
    public static void clearOnDeath(EntityMaid maid) {
        if (maid != null) {
            clear(maid);
        }
    }

    private static void clear(EntityMaid maid) {
        maid.getPersistentData().putBoolean(TAG_UNCONSCIOUS, false);
        maid.getPersistentData().putInt(TAG_RECOVERY, 0);
        markSynced(maid, false);
        clearGroundLift(maid);
        ACTIVE.remove(maid.getStringUUID());
    }

    /** 停服、换存档：只清服务端索引，实体 NBT 原样保留。 */
    public static void reset() {
        ACTIVE.clear();
        discoverClock = 0;
    }

    /** 致命伤结算后、死亡判定前拉回 1 点血并进入昏迷。 */
    public static void onDamage(LivingDamageEvent.Post event) {
        if (!(event.getEntity() instanceof EntityMaid maid)) {
            return;
        }
        if (maid.level().isClientSide() || maid.isRemoved()) {
            return;
        }
        if (maid.getHealth() > 0.0F || isUnconscious(maid) || isFatalSource(event.getSource())) {
            return;
        }
        enter(maid, event.getSource());
    }

    /** 昏迷期间普通伤害一律不结算；只有 /kill 与虚空继续往下走。 */
    public static void onIncoming(LivingIncomingDamageEvent event) {
        if (!(event.getEntity() instanceof EntityMaid maid)) {
            return;
        }
        if (isUnconscious(maid) && !isFatalSource(event.getSource())) {
            event.setCanceled(true);
        }
    }

    /** 真死亡时清掉昏迷标记；被别的模组取消的死亡不清。 */
    public static void onDeath(LivingDeathEvent event) {
        if (event.isCanceled()) {
            return;
        }
        if (event.getEntity() instanceof EntityMaid maid) {
            clearOnDeath(maid);
        }
    }

    private static boolean isFatalSource(DamageSource source) {
        return source.is(DamageTypes.GENERIC_KILL) || source.is(DamageTypes.FELL_OUT_OF_WORLD);
    }

    private static void heart(EntityMaid maid) {
        if (maid.level() instanceof ServerLevel level) {
            level.sendParticles(
                    ParticleTypes.HEART,
                    maid.getX(),
                    maid.getY() + 0.8,
                    maid.getZ(),
                    1,
                    0.25,
                    0.15,
                    0.25,
                    0.0);
        }
    }

    private static void heartsAround(EntityMaid maid, int count) {
        if (maid.level() instanceof ServerLevel level) {
            level.sendParticles(
                    ParticleTypes.HEART,
                    maid.getX(),
                    maid.getY() + 0.9,
                    maid.getZ(),
                    count,
                    0.4,
                    0.25,
                    0.4,
                    0.0);
        }
    }

    private static void broadcastUnconscious(EntityMaid maid, DamageSource source, boolean onBed) {
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge == null || !bridge.hasClients()) {
            return;
        }
        Entity attacker = Attackers.actor(source);
        Entity owner = maid.getOwner();
        JsonObject frame = EventFrames.base("maid_unconscious", maid.getStringUUID());
        frame.addProperty("attacker", Attackers.name(source));
        frame.addProperty("attacker_type", Attackers.typeId(source));
        frame.addProperty("attacker_owner", attacker == owner && owner != null);
        frame.addProperty("attacker_maid", attacker == maid);
        frame.addProperty("damage_type", source.getMsgId());
        frame.addProperty("on_bed", onBed);
        bridge.broadcastEvent(frame);
    }

    private static void broadcastRecovering(EntityMaid maid) {
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge == null || !bridge.hasClients()) {
            return;
        }
        JsonObject frame = EventFrames.base("maid_recovering", maid.getStringUUID());
        frame.addProperty("seconds", RECOVERY_TICKS / 20);
        bridge.broadcastEvent(frame);
    }

    private static void broadcastRecovered(EntityMaid maid) {
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge == null || !bridge.hasClients()) {
            return;
        }
        JsonObject frame = EventFrames.base("maid_recovered", maid.getStringUUID());
        frame.addProperty("health", maid.getHealth());
        bridge.broadcastEvent(frame);
    }
}
