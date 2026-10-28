package com.lingchat.maidbridge.action.attack;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.EventFrames;
import com.lingchat.maidbridge.perception.nearby.CreatureMemory;
import java.util.ArrayDeque;
import java.util.Deque;
import java.util.List;
import java.util.UUID;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
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
 * <p>单槽位、队列推进：一次可以点好几只，按顺序一只一只打完。新指令顶掉旧的那次；
 * 动作、工作模式这些指令也会把它收掉——同时有两套东西在写她的寻路，她会在两个目标之间
 * 来回走。
 *
 * <p>目标不见了的时候去问 {@link CreatureMemory}：是她自己打死的（这条算打完）、被别人
 * 先打死的，还是跑远了。说不清原因时也要说，否则模型以为她还在打。
 *
 * <p>只在服务端主线程上跑（请求经 MessageRouter 排队、推进挂在 tick 事件上），状态不需要同步。
 */
public final class MaidAttackController {
    /** 追一只这么久还没打倒就放弃（tick，20 = 1 秒）。目标跑远、绕不过去时不留悬案。 */
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
        final ServerLevel level;
        final boolean singleHit;
        /** 还没打完的目标，队首是正在追的那只。 */
        final Deque<UUID> targets = new ArrayDeque<>();
        int ticks;
        int cooldown;

        Run(EntityMaid maid, boolean singleHit) {
            this.maidId = maid.getStringUUID();
            this.level = (ServerLevel) maid.level();
            this.singleHit = singleHit;
        }
    }

    private static Run current;

    private MaidAttackController() {
    }

    /** 受理一次攻击：记下她与目标队列，清掉残留的路径，后面每 tick 推进。 */
    public static void start(EntityMaid maid, List<LivingEntity> targets, boolean singleHit) {
        if (targets.isEmpty()) {
            return;
        }
        Run run = new Run(maid, singleHit);
        long tick = maid.level().getGameTime();
        for (LivingEntity target : targets) {
            run.targets.add(target.getUUID());
            // 记住它：追到一半它没了的时候，得说得出是死了还是走了
            CreatureMemory.observe(target, maid, tick);
        }
        current = run;
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

    /** 这只女仆正在追打这个目标吗；轻轻一下不算，那几下就收手。 */
    public static boolean isFighting(EntityMaid maid, LivingEntity target) {
        Run run = current;
        return run != null && !run.singleHit && maid != null && target != null
                && run.maidId.equals(maid.getStringUUID())
                && run.targets.contains(target.getUUID());
    }

    /** 每个服务端 tick 推进一次（主线程）。 */
    public static void tick(MinecraftServer server) {
        Run run = current;
        if (run == null) {
            return;
        }
        EntityMaid maid = MaidQueries.findMaidById(server, run.maidId);
        if (maid == null || !maid.isAlive()) {
            current = null;
            return;
        }
        // 换了维度：她跟过去了，原来那个目标已经不在她这边
        if (maid.level() != run.level) {
            reportFailure(maid, run, run.targets.peek(), "target_left_range");
            stop(maid);
            return;
        }
        // 被玩家抱着时打不了，也不能把她从手上拽下来：这一仗只能停在这里
        if (MaidQueries.carriedByPlayer(maid)) {
            reportFailure(maid, run, run.targets.peek(), "maid_carried");
            stop(maid);
            return;
        }
        // 她正坐着、睡着或坐在座位上时走不动，先叫起来
        if (maid.isSleeping() || maid.isMaidInSittingPose() || maid.isPassenger()) {
            MaidActuator.standUp(maid);
        }
        UUID targetId = run.targets.peek();
        Entity resolved = run.level.getEntity(targetId);
        if (!(resolved instanceof LivingEntity target) || !target.isAlive() || target.isSpectator()) {
            settle(maid, run, targetId);
            return;
        }
        if (++run.ticks > TIMEOUT_TICKS) {
            reportFailure(maid, run, targetId, "timeout");
            advance(maid, run);
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
            // 轻轻一下：固定伤害，打一下就换下一只，不进入冷却循环
            target.hurt(maid.damageSources().mobAttack(maid), TAP_DAMAGE);
            advance(maid, run);
            return;
        }
        maid.doHurtTarget(target);
        run.cooldown = cooldownTicks(maid);
    }

    /**
     * 目标不见了：查记得的活物，分清是她自己打死的（算打完）、被别人先打死，还是跑了。
     *
     * <p>她自己打死的不用报：附近死亡那条会把它说成「被女仆杀了」，这里再补一句就是重复。
     */
    private static void settle(EntityMaid maid, Run run, UUID targetId) {
        CreatureMemory.Known known = CreatureMemory.lookup(targetId.toString(), maid.level().getGameTime());
        if (known != null && known.dead() && "maid".equals(known.killedBy())) {
            advance(maid, run);
            return;
        }
        String code;
        if (known != null && known.dead()) {
            code = switch (known.killedBy()) {
                case "owner" -> "killed_by_owner";
                case "other" -> "killed_by_other";
                default -> "killed_unknown";
            };
        } else {
            code = "target_left_range";
        }
        reportFailure(maid, run, targetId, code);
        advance(maid, run);
    }

    /** 这一只打完了（或放弃了）：换下一只，全部做完就收手。 */
    private static void advance(EntityMaid maid, Run run) {
        run.targets.poll();
        run.ticks = 0;
        run.cooldown = 0;
        if (run.targets.isEmpty()) {
            stop(maid);
        }
    }

    /** 收工：清掉循环与她身上由我们写进去的寻路/视线。 */
    private static void stop(EntityMaid maid) {
        current = null;
        MaidActuator.stopWalking(maid);
        MaidActuator.stopLooking(maid);
    }

    /**
     * 追打没能打完：推一条事件，插件据此补一句「那只僵尸没能打完，因为…」。
     *
     * <p>目标已经查不到了，类型与称呼从记忆里取；记忆也没有就只报原因码。
     */
    private static void reportFailure(EntityMaid maid, Run run, UUID targetId, String code) {
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge == null || !bridge.hasClients() || targetId == null) {
            return;
        }
        CreatureMemory.Known known = CreatureMemory.lookup(targetId.toString(), maid.level().getGameTime());
        ServerPlayer owner = MaidQueries.owner(maid);
        JsonObject frame = EventFrames.base("attack_failed", maid.getStringUUID());
        frame.addProperty("reason_code", code);
        frame.addProperty("remaining", Math.max(0, run.targets.size() - 1));
        // 死在别的生物手里时把那只生物也带上，插件能说成「被骷髅打死了」而不是「被别的东西」
        frame.addProperty("killed_by_type", known == null ? "" : known.killerType());
        JsonObject target = new JsonObject();
        target.addProperty("id", targetId.toString());
        target.addProperty("type", known == null ? "" : known.type());
        target.addProperty("category", known == null ? "" : known.category());
        target.addProperty("name", known == null ? "" : known.name());
        target.addProperty("distance", EventFrames.round(known == null ? -1.0 : known.distance()));
        target.addProperty("owner", owner != null && targetId.equals(owner.getUUID()));
        frame.add("target", target);
        bridge.broadcastEvent(frame);
    }

    /** 与车万女仆的攻击冷却同一口径：攻击速度越高出手越快。 */
    private static int cooldownTicks(EntityMaid maid) {
        double speed = maid.getAttributeValue(Attributes.ATTACK_SPEED);
        return speed > 0 ? Math.max(MIN_COOLDOWN_TICKS, (int) (20 / speed)) : 20;
    }
}
