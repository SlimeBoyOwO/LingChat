package com.lingchat.maidbridge.action.nav;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.core.MaidActuator;
import java.util.List;
import java.util.function.Predicate;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.item.Item;
import net.minecraft.world.phys.Vec3;

/**
 * 把女仆送到目标位置：走、跳下、跑跳、垫脚、搭路、出屋，按环境和额度自己挑。
 *
 * <p>调用方只给目标与边界（不许垫哪里、料从哪来），不关心用哪种手段。每一次目标是一个
 * 小状态机：没有腿在跑就重新规划，选中一条腿就跑到它结束；腿失败记进记忆，换下一种。
 * 腿的进出一律走 enter / tick / exit，跑跳结束会还重力，垫脚结束会记账。
 *
 * <p>规划由 {@link RouteSearch} 在落脚点图上一次算出一条动作序列：走、跳下、跑跳、
 * 搭一格、垫一格各有自己的代价，搜索自己组合，不再按写死的顺序逐个试。序列一段一段
 * 执行，哪一段失败就整条作废重算。
 */
public final class Navigator {
    public enum Status {
        RUNNING,
        ARRIVED,
        FAILED
    }

    /** 一次目标最多换这么多次腿都不成功，就判定走不通。 */
    private static final int MAX_LEG_FAILURES = 5;
    /** 离目标一直不再靠近这么久（约四十五秒）才判超时：绕路、搭一段再走回来都不算白耗。 */
    private static final int NO_PROGRESS_TICKS = 900;
    /** 距离比上次的最好成绩近了这么多，才算有进展。 */
    private static final double PROGRESS_STEP = 0.5;
    /** 同一个目标最多从高处直落几次。 */
    private static final int MAX_DROPS = 2;
    /** 原地起跳的初速：与玩家跳跃一致，刚好够她升过一整格。 */
    private static final double JUMP_POWER = 0.42;
    /** 起跳后最多等这么久：没离地、也没把方块补进去就放手。 */
    private static final int JUMP_TIMEOUT_TICKS = 20;
    /** 垫完一块之后歇这么久再判下一步，让她先落回新方块上。 */
    private static final int JUMP_LAND_TICKS = 10;
    /** 收尾时实在没有落脚点可去，等这么久就直接拆脚下的垫脚块。 */
    private static final int SETTLE_WAIT_TICKS = 30;

    private final NavGoal goal;
    private final NavPolicy policy;
    private final NavMemory memory;
    private final ScaffoldBook book;

    private Leg leg;
    private Leg queued;
    private Status status = Status.RUNNING;
    private NavFailure failure;
    private NavFailure lastLegFailure = NavFailure.NO_PATH;
    private BlockPos stand;
    private String phase = "规划";
    private int legFailures;
    private int restTicks;
    private int settleWaits;

    /** 正在算的路线图搜索；算到路线就换成下面那一串动作。 */
    private RouteSearch search;
    /** 已经算好的动作序列，一段一段执行；失败或走完就丢掉重算。 */
    private List<RouteSearch.Step> route = List.of();
    private int routeAt;
    /** 有进展的判据：离目标最近到过哪里，以及多久没再靠近。 */
    private double bestDistance = Double.MAX_VALUE;
    private int stillTicks;

    private Navigator(NavGoal goal, NavPolicy policy, NavMemory memory, ScaffoldBook book) {
        this.goal = goal;
        this.policy = policy;
        this.memory = memory;
        this.book = book;
    }

    public static Navigator start(
            NavGoal goal, NavPolicy policy, NavMemory memory, ScaffoldBook book) {
        return new Navigator(goal, policy, memory, book);
    }

    public Status tick(EntityMaid maid) {
        if (status != Status.RUNNING) {
            return status;
        }
        if (restTicks > 0) {
            restTicks--;
            return status;
        }
        // 空中不算到站：跑跳弧线经过目标上方时也算得着，但人还没落地
        if (maid.onGround() && satisfied(maid)) {
            endLeg(maid);
            return arrive(maid);
        }
        if (stalled(maid)) {
            endLeg(maid);
            return fail(NavFailure.TIMEOUT);
        }
        if (leg == null) {
            plan(maid);
            if (status != Status.RUNNING) {
                return status;
            }
            if (queued != null) {
                leg = queued;
                queued = null;
                leg.enter(this, maid);
            }
            if (leg == null) {
                // 路线还在算：这个 tick 先站着，不算空转
                return status;
            }
        }
        Leg.Outcome outcome = leg.tick(this, maid);
        if (outcome == Leg.Outcome.RUNNING) {
            return status;
        }
        Leg finished = leg;
        leg = null;
        finished.exit(this, maid);
        if (outcome == Leg.Outcome.FAILED) {
            moveFailed(lastLegFailure);
            if (status != Status.RUNNING) {
                return status;
            }
        } else {
            legFailures = 0;
        }
        if (queued != null) {
            leg = queued;
            queued = null;
            leg.enter(this, maid);
        }
        return status;
    }

    /** 收手：停下、还重力，垫出来的方块由调用方决定拆不拆。 */
    public void cancel(EntityMaid maid) {
        endLeg(maid);
        queued = null;
        search = null;
        route = List.of();
        routeAt = 0;
        restTicks = 0;
        MaidActuator.stopWalking(maid);
        status = Status.FAILED;
    }

    /** 结束当前这条腿：跑跳的退出会把重力还回来，别把人留在半空。 */
    private void endLeg(EntityMaid maid) {
        if (leg != null) {
            leg.exit(this, maid);
            leg = null;
        }
    }

    public Status status() {
        return status;
    }

    public NavFailure failure() {
        return failure;
    }

    public BlockPos stand() {
        return stand;
    }

    public String phase() {
        return phase;
    }

    public List<BlockPos> chain() {
        return leg == null ? List.of() : leg.chain();
    }

    public ScaffoldBook book() {
        return book;
    }

    /** 空闲时顺手拆掉她走开的垫脚块；正在赶路时先护着。 */
    public void cleanScaffold(EntityMaid maid) {
        if (maid.level() instanceof ServerLevel level) {
            book.clean(level, maid, stand, false, leg != null, policy.consumeMaterials());
        }
    }

    private Status arrive(EntityMaid maid) {
        MaidActuator.stopWalking(maid);
        stand = maid.blockPosition().immutable();
        status = Status.ARRIVED;
        return status;
    }

    private Status fail(NavFailure why) {
        failure = why;
        lastLegFailure = why;
        status = Status.FAILED;
        return status;
    }

    /**
     * 这条腿没成：整条路线作废，下一 tick 从她此刻的位置重算；连续失败太多次才判死。
     */
    private void moveFailed(NavFailure why) {
        lastLegFailure = why;
        route = List.of();
        routeAt = 0;
        if (++legFailures > MAX_LEG_FAILURES) {
            fail(why);
        }
    }

    private boolean satisfied(EntityMaid maid) {
        BlockPos here = maid.blockPosition();
        if (goal instanceof NavGoal.Near near) {
            return here.distManhattan(near.target()) <= near.closeEnough();
        }
        if (!(goal instanceof NavGoal.Stand target)) {
            return false;
        }
        // 站在目标格里不算到：那一格是要放东西的地方
        return !here.equals(target.target()) && target.canActFrom().test(here);
    }

    /** 这一次目标要去的格子：站位是目标本身，走近是玩家所在那一格。 */
    private BlockPos goalTarget() {
        if (goal instanceof NavGoal.Stand stand) {
            return stand.target();
        }
        if (goal instanceof NavGoal.Near near) {
            return near.target();
        }
        return null;
    }

    /** 图搜索认不认这一格是终点。 */
    private Predicate<BlockPos> goalTest() {
        if (goal instanceof NavGoal.Stand stand) {
            return pos -> !pos.equals(stand.target()) && stand.canActFrom().test(pos);
        }
        NavGoal.Near near = (NavGoal.Near) goal;
        return pos -> pos.distManhattan(near.target()) <= near.closeEnough();
    }

    /**
     * 离目标一直没更近才算超时。
     *
     * <p>绕路、从屋外爬上去、先搭一段再走回来，距离都可能一时变远；只盯着「最近到过
     * 哪里」，只要她在靠近就不打断。
     */
    private boolean stalled(EntityMaid maid) {
        BlockPos target = goalTarget();
        if (target == null) {
            return false;
        }
        double distance = maid.position().distanceToSqr(Vec3.atCenterOf(target));
        if (distance < bestDistance - PROGRESS_STEP) {
            bestDistance = distance;
            stillTicks = 0;
            return false;
        }
        return ++stillTicks > NO_PROGRESS_TICKS;
    }

    /** 没有腿在跑时推进规划：先续上算好的路线，再不行就重算一次。 */
    private void plan(EntityMaid maid) {
        phase = "规划";
        if (!(maid.level() instanceof ServerLevel level)) {
            fail(NavFailure.NO_STAND);
            return;
        }
        if (goal instanceof NavGoal.Settle) {
            planSettle(maid, level);
            return;
        }
        // 上一条路线还没走完：接着执行下一段，不必每走一步都重算
        if (routeAt < route.size()) {
            execute(maid, route.get(routeAt++));
            return;
        }
        route = List.of();
        routeAt = 0;
        BlockPos target = goalTarget();
        if (target == null) {
            fail(NavFailure.NO_STAND);
            return;
        }
        if (search == null) {
            // 上一次腿失败留下的「走不到」不代表这条路走不通：新一趟重新算
            MaidActuator.beginWalk(maid);
            search = RouteSearch.start(
                    new RouteView(maid, target, goalTest(), policy, memory),
                    maid.blockPosition(), target, limits());
        }
        RouteSearch.State state = search.step();
        if (state == RouteSearch.State.SEARCHING) {
            return;
        }
        if (state == RouteSearch.State.EXHAUSTED) {
            search = null;
            fail(canJumpHere(maid) ? NavFailure.NO_STAND : NavFailure.NO_HEADROOM);
            return;
        }
        route = search.route();
        search = null;
        if (route.isEmpty()) {
            fail(NavFailure.NO_STAND);
            return;
        }
        execute(maid, route.get(routeAt++));
    }

    /** 这一次目标还剩多少额度可以动地形。 */
    private RouteSearch.Limits limits() {
        return new RouteSearch.Limits(
                Math.max(0, ScaffoldRules.MAX_BLOCKS - book.size()),
                Math.max(0, ScaffoldRules.MAX_STEPS - memory.steps),
                Math.max(0, ScaffoldRules.MAX_BRIDGE - memory.bridges),
                Math.max(0, MAX_DROPS - memory.drops));
    }

    /** 执行路线的一段：走、跳、搭、垫，或者原地起跳。 */
    private void execute(EntityMaid maid, RouteSearch.Step step) {
        switch (step.kind()) {
            case WALK -> startWalk(maid, step.to());
            case HOP -> hop(maid, step.to(), ParkourRules.MAX_FALL, "跑跳过缺口");
            case DROP -> {
                memory.drops++;
                hop(maid, step.to(), ParkourRules.DEEP_FALL, "跳下");
            }
            case BRIDGE -> bridge(maid, step.to());
            case RISE -> rise(maid, step);
            case PILLAR -> pillar(maid);
        }
    }

    /** 收工：走到实地上，把垫脚拆净。 */
    private void planSettle(EntityMaid maid, ServerLevel level) {
        phase = "收尾";
        if (!book.standingOn(maid)) {
            book.clearNow(level, maid, policy.consumeMaterials());
            arrive(maid);
            return;
        }
        BlockPos away = awayStand(maid, level);
        if (away != null) {
            startWalk(maid, away);
            return;
        }
        // 没有别的落脚点：等一小会儿，连脚下的垫脚块一起拆掉，让她落回地面
        if (++settleWaits > SETTLE_WAIT_TICKS) {
            book.clearNow(level, maid, policy.consumeMaterials());
            arrive(maid);
        }
    }

    private void startWalk(EntityMaid maid, BlockPos to) {
        stand = to;
        phase = "走向落脚点";
        queued = new WalkLeg(to);
    }

    private void push(Leg next, String label) {
        phase = label;
        queued = next;
    }

    /** 跑跳或直落：一段弧线过去。 */
    private void hop(EntityMaid maid, BlockPos to, int maxFall, String label) {
        MaidParkour.Runner runner = new MaidParkour.Runner(maid, List.of(to), maxFall);
        if (runner.failed()) {
            memory.triedStands.add(to);
            moveFailed(NavFailure.NO_PATH);
            return;
        }
        stand = null;
        push(new HopLeg(runner, label), label);
    }

    /** 平着搭一格：在下一格脚底补方框，走过去。 */
    private void bridge(EntityMaid maid, BlockPos to) {
        Item item = ScaffoldBook.item(maid, policy.consumeMaterials());
        if (item == null
                || !book.put((ServerLevel) maid.level(), maid, to.below(), item,
                        policy.consumeMaterials())) {
            memory.badBlocks.add(to.below());
            moveFailed(NavFailure.NO_STAND);
            return;
        }
        memory.bridges++;
        startWalk(maid, to);
    }

    /** 垫一格：先把托底补上，再放顶块，然后走上去。 */
    private void rise(EntityMaid maid, RouteSearch.Step step) {
        ServerLevel level = (ServerLevel) maid.level();
        Item item = ScaffoldBook.item(maid, policy.consumeMaterials());
        BlockPos block = step.block();
        if (item == null) {
            moveFailed(NavFailure.NO_STAND);
            return;
        }
        if (step.support()
                && !book.put(level, maid, block.below(), item, policy.consumeMaterials())) {
            memory.badBlocks.add(block.below());
            moveFailed(NavFailure.NO_STAND);
            return;
        }
        if (!book.put(level, maid, block, item, policy.consumeMaterials())) {
            memory.badBlocks.add(block);
            moveFailed(NavFailure.NO_STAND);
            return;
        }
        memory.steps++;
        startWalk(maid, step.to());
    }

    /** 原地起跳，把脚下这一格补上。 */
    private void pillar(EntityMaid maid) {
        BlockPos feet = maid.blockPosition().immutable();
        Item item = ScaffoldBook.item(maid, policy.consumeMaterials());
        BlockPos target = goalTarget();
        if (item == null || target == null || !jumpUp(maid, feet, target, item)) {
            memory.badBlocks.add(feet);
            moveFailed(NavFailure.NO_STAND);
        }
    }

    /** 原地跳起来把脚下的方块补上，落下来就升了一整格。 */
    private boolean jumpUp(EntityMaid maid, BlockPos feet, BlockPos target, Item item) {
        // 跳起来会把落脚点抬高一层：抬高后正好是目标格就别跳，那是要放方块的地方
        if (feet.above().equals(target)) {
            return false;
        }
        if (!maid.onGround()
                || !policy.canBuildOn(feet)
                || !Terrain.free(maid.level(), feet)
                || !Terrain.free(maid.level(), feet.above())
                || !Terrain.free(maid.level(), feet.above(2))
                || !Terrain.solid(maid.level(), feet.below())
                || Bodies.other(maid.level(), feet, maid)) {
            return false;
        }
        MaidActuator.stopWalking(maid);
        maid.setDeltaMovement(0.0, JUMP_POWER, 0.0);
        // 人一离地，脚下这块就不在碰撞盒里了：先把落脚格记下来，收尾时护着它
        stand = feet.above().immutable();
        queued = new JumpLeg(feet.immutable(), item);
        phase = "原地起跳垫高";
        return true;
    }

    /** 她此刻能不能起跳：脚与头两格之外，头顶上面还要再让出一格。 */
    private boolean canJumpHere(EntityMaid maid) {
        BlockPos feet = maid.blockPosition();
        return Terrain.free(maid.level(), feet.above())
                && Terrain.free(maid.level(), feet.above(2));
    }

    /** 收工时去哪站着：最近的一格能站人、又不踩着垫脚块的地面。 */
    private BlockPos awayStand(EntityMaid maid, ServerLevel level) {
        BlockPos here = maid.blockPosition();
        BlockPos best = null;
        double bestScore = Double.MAX_VALUE;
        for (int dx = -6; dx <= 6; dx++) {
            for (int dz = -6; dz <= 6; dz++) {
                for (int dy = -2; dy <= 2; dy++) {
                    BlockPos spot = here.offset(dx, dy, dz);
                    if (book.contains(spot) || book.contains(spot.below())) {
                        continue;
                    }
                    if (!level.isLoaded(spot) || !level.isLoaded(spot.above())) {
                        continue;
                    }
                    if (!Terrain.free(level, spot) || !Terrain.free(level, spot.above())) {
                        continue;
                    }
                    if (!Terrain.solid(level, spot.below())) {
                        continue;
                    }
                    double score = spot.distSqr(here);
                    if (score < bestScore) {
                        bestScore = score;
                        best = spot;
                    }
                }
            }
        }
        return best;
    }

    // ---------------- 腿 ----------------

    private interface Leg {
        enum Outcome {
            RUNNING,
            DONE,
            FAILED
        }

        Outcome tick(Navigator nav, EntityMaid maid);

        default void enter(Navigator nav, EntityMaid maid) {
        }

        default void exit(Navigator nav, EntityMaid maid) {
        }

        default List<BlockPos> chain() {
            return List.of();
        }
    }

    /** 走向落脚点：到了就结束，卡住就记下这个站位。 */
    private final class WalkLeg implements Leg {
        private final BlockPos to;
        private int ticks;
        private int still;
        private Vec3 last;

        private WalkLeg(BlockPos to) {
            this.to = to;
        }

        @Override
        public void enter(Navigator nav, EntityMaid maid) {
            last = maid.position();
            MaidActuator.walkToPos(maid, to, 0);
        }

        @Override
        public Outcome tick(Navigator nav, EntityMaid maid) {
            if (maid.blockPosition().equals(to)) {
                MaidActuator.stopWalking(maid);
                stand = to;
                return Outcome.DONE;
            }
            // 绕路时离目标变远是正常的，只看她自己有没有真的在动
            Vec3 now = maid.position();
            if (last != null && now.distanceToSqr(last) < 0.0025) {
                still++;
            } else {
                still = 0;
            }
            last = now;
            boolean noPath = ++ticks > 80
                    && maid.getBrain().hasMemoryValue(MemoryModuleType.CANT_REACH_WALK_TARGET_SINCE);
            if (noPath || still > 80) {
                memory.triedStands.add(to);
                lastLegFailure = NavFailure.NO_PATH;
                MaidActuator.stopWalking(maid);
                return Outcome.FAILED;
            }
            MaidActuator.walkToPos(maid, to, 0);
            return Outcome.RUNNING;
        }

        @Override
        public void exit(Navigator nav, EntityMaid maid) {
            MaidActuator.stopWalking(maid);
        }
    }

    /** 跑跳：一段一段沿验过的路线过去。 */
    private final class HopLeg implements Leg {
        private final MaidParkour.Runner runner;
        private final String label;

        private HopLeg(MaidParkour.Runner runner, String label) {
            this.runner = runner;
            this.label = label;
        }

        @Override
        public void enter(Navigator nav, EntityMaid maid) {
            phase = label;
        }

        @Override
        public Outcome tick(Navigator nav, EntityMaid maid) {
            runner.tick();
            if (runner.failed()) {
                memory.triedStands.addAll(runner.chain());
                lastLegFailure = NavFailure.NO_PATH;
                return Outcome.FAILED;
            }
            if (!runner.done()) {
                return Outcome.RUNNING;
            }
            stand = maid.blockPosition().immutable();
            return Outcome.DONE;
        }

        @Override
        public void exit(Navigator nav, EntityMaid maid) {
            if (!runner.done() && !runner.failed()) {
                runner.cancel();
            }
        }

        @Override
        public List<BlockPos> chain() {
            return runner.chain();
        }
    }

    /** 原地起跳：跳起来把脚下的方块补上，落下来升了一整格。 */
    private final class JumpLeg implements Leg {
        private final BlockPos pos;
        private final Item item;
        private int ticks;

        private JumpLeg(BlockPos pos, Item item) {
            this.pos = pos;
            this.item = item;
        }

        @Override
        public Outcome tick(Navigator nav, EntityMaid maid) {
            if (++ticks > JUMP_TIMEOUT_TICKS) {
                // 跳不起来（头顶被封、被别的模组按住）：这一格换别的办法
                lastLegFailure = NavFailure.NO_HEADROOM;
                return Outcome.FAILED;
            }
            if (maid.onGround()) {
                if (!maid.blockPosition().equals(pos) || ticks > 3) {
                    // 被挤开了，或者推了几下都没离地
                    lastLegFailure = NavFailure.NO_STAND;
                    return Outcome.FAILED;
                }
                maid.setDeltaMovement(0.0, JUMP_POWER, 0.0);
                return Outcome.RUNNING;
            }
            if (maid.getY() < pos.getY() + 1.02) {
                return Outcome.RUNNING;
            }
            if (!book.put((ServerLevel) maid.level(), maid, pos, item,
                    policy.consumeMaterials())) {
                memory.badBlocks.add(pos);
                lastLegFailure = NavFailure.NO_STAND;
                return Outcome.FAILED;
            }
            memory.steps++;
            stand = pos.above().immutable();
            restTicks = JUMP_LAND_TICKS;
            return Outcome.DONE;
        }
    }
}
