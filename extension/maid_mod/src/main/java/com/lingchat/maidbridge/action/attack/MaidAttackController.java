package com.lingchat.maidbridge.action.attack;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.maid.MaidQueries;
import java.util.UUID;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.ai.attributes.Attributes;

/**
 * 攻击指令的执行者：追上去、贴身、按攻击速度一下一下地打。
 *
 * <p>为什么不用车万女仆自己的攻击 AI：那套行为只在她处于 <b>WORK 日程 + 攻击模式</b>时才会
 * 跑，玩家半夜喊一句「打它」她只会站着；而且它的目标有效范围按居家/跟随模式算，稍远一点
 * 就把目标清掉。这里自己驱动（写 {@code WALK_TARGET} + 直接 {@code doHurtTarget}），
 * 跟她当前在干什么活、什么作息都无关。
 *
 * <p>单槽位：新的攻击令顶掉旧的那次。动作、工作模式这些指令也会把它收掉——同时有两套东西
 * 在写她的寻路，她会在两个目标之间来回走。
 *
 * <p>只在服务端主线程上跑（请求经 MessageRouter 排队、推进挂在 tick 事件上），状态不需要同步。
 */
public final class MaidAttackController {
    /** 追这么久还没打倒就放弃（tick，20 = 1 秒）。目标跑远、绕不过去时不留悬案。 */
    private static final int TIMEOUT_TICKS = 600;
    /** 两次出手之间至少隔这么多 tick：攻击速度受属性影响，但总得让她抬手。 */
    private static final int MIN_COOLDOWN_TICKS = 4;
    /** 走到离目标这么近就停下开打。 */
    private static final int CLOSE_ENOUGH = 1;
    /**
     * 「轻轻打一下」的固定伤害。
     *
     * <p>不走 {@code doHurtTarget}：那一下按她手上的武器算，玩家只是想逗她抬手的话，
     * 一把好武器一下就能打掉大半条命。这里固定半点血，穿着甲会更少。
     */
    private static final float TAP_DAMAGE = 1.0F;

    private static final class Run {
        final String maidId;
        final UUID targetId;
        final ServerLevel level;
        final boolean singleHit;
        int ticks;
        int cooldown;

        Run(EntityMaid maid, LivingEntity target, boolean singleHit) {
            this.maidId = maid.getStringUUID();
            this.targetId = target.getUUID();
            this.level = (ServerLevel) maid.level();
            this.singleHit = singleHit;
        }
    }

    private static Run current;

    private MaidAttackController() {
    }

    /** 受理一次攻击：记下她与目标，清掉残留的路径，后面每 tick 推进。 */
    public static void start(EntityMaid maid, LivingEntity target, boolean singleHit) {
        current = new Run(maid, target, singleHit);
        MaidActuator.beginWalk(maid);
    }

    /** 被别的指令顶掉，或停服时清干净。 */
    public static void cancel() {
        current = null;
    }

    /** 这只女仆正在被我们追打：跟随行为给长任务让路时用。 */
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
        Entity resolved = run.level.getEntity(run.targetId);
        if (maid == null || !maid.isAlive() || maid.level() != run.level
                || !(resolved instanceof LivingEntity target) || !target.isAlive() || target.isSpectator()) {
            // 目标没了（多半是被打死了）就收工：死亡旁白会告诉插件，这里不必再说什么
            current = null;
            return;
        }
        // 被玩家抱着时打不了，也不能把她从手上拽下来：直接收工，等放下再说
        if (MaidQueries.carriedByPlayer(maid)) {
            current = null;
            return;
        }
        // 她正坐着、睡着或坐在座位上时走不动，先叫起来
        if (maid.isSleeping() || maid.isMaidInSittingPose() || maid.isPassenger()) {
            MaidActuator.standUp(maid);
        }
        if (++run.ticks > TIMEOUT_TICKS) {
            finish(maid);
            return;
        }
        if (!maid.isWithinMeleeAttackRange(target)) {
            MaidActuator.walkToEntity(maid, target, CLOSE_ENOUGH);
            return;
        }
        MaidActuator.stopWalking(maid);
        MaidActuator.lookAtEntity(maid, target);
        if (--run.cooldown > 0) {
            return;
        }
        maid.swing(InteractionHand.MAIN_HAND);
        if (run.singleHit) {
            // 轻轻一下：固定伤害，打一下就收手，不进入冷却循环
            target.hurt(maid.damageSources().mobAttack(maid), TAP_DAMAGE);
            finish(maid);
            return;
        }
        maid.doHurtTarget(target);
        run.cooldown = cooldownTicks(maid);
    }

    /** 收工：清掉循环与她身上由我们写进去的寻路/视线。 */
    private static void finish(EntityMaid maid) {
        current = null;
        MaidActuator.stopWalking(maid);
        MaidActuator.stopLooking(maid);
    }

    /** 与车万女仆的攻击冷却同一口径：攻击速度越高出手越快。 */
    private static int cooldownTicks(EntityMaid maid) {
        double speed = maid.getAttributeValue(Attributes.ATTACK_SPEED);
        return speed > 0 ? Math.max(MIN_COOLDOWN_TICKS, (int) (20 / speed)) : 20;
    }
}
