package com.lingchat.maidbridge.action.nav;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.SearchBudget;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.action.place.Placement;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Predicate;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.Level;
import net.minecraft.world.phys.Vec3;

/**
 * 把女仆送到目标位置：走、跳下、跑跳、垫脚、搭路、出屋，按环境和预算自己挑。
 *
 * <p>调用方只给目标与边界（不许垫哪里、料从哪来），不关心用哪种手段。每一次目标是一个
 * 小状态机：没有腿在跑就重新规划，选中一条腿就跑到它结束；腿失败记进记忆，换下一种。
 * 腿的进出一律走 enter / tick / exit，跑跳结束会还重力，垫脚结束会记账。
 */
public final class Navigator {
    public enum Status {
        RUNNING,
        ARRIVED,
        FAILED
    }

    /** 一次目标最多换这么多次腿都不成功，就判定走不通。 */
    private static final int MAX_LEG_FAILURES = 5;
    /** 一次目标最多耗这么久（约半分钟）。 */
    private static final int MAX_TICKS = 600;
    /** 连续这么多 tick 一个可行的动作都排不出来，就判定走不通。 */
    private static final int MAX_IDLE_TICKS = 40;
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
    private int ticks;
    private int legFailures;
    private int idleTicks;
    private int restTicks;
    private int settleWaits;

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

    public Status tick(EntityMaid maid, SearchBudget budget) {
        if (status != Status.RUNNING) {
            return status;
        }
        if (++ticks > MAX_TICKS) {
            return fail(NavFailure.TIMEOUT);
        }
        if (restTicks > 0) {
            restTicks--;
            return status;
        }
        if (satisfied(maid)) {
            return arrive(maid);
        }
        if (leg == null) {
            plan(maid, budget);
            if (status != Status.RUNNING) {
                return status;
            }
            if (queued != null) {
                leg = queued;
                queued = null;
                leg.enter(this, maid);
            }
            if (leg == null) {
                // 本 tick 一个可行的动作都没排出来：等几 tick；一直排不出来就是走不通
                if (++idleTicks > MAX_IDLE_TICKS) {
                    return fail(canJumpHere(maid) ? NavFailure.NO_STAND : NavFailure.NO_HEADROOM);
                }
                return status;
            }
            idleTicks = 0;
        }
        Leg.Outcome outcome = leg.tick(this, maid);
        if (outcome == Leg.Outcome.RUNNING) {
            return status;
        }
        Leg finished = leg;
        leg = null;
        finished.exit(this, maid);
        if (outcome == Leg.Outcome.FAILED) {
            if (++legFailures > MAX_LEG_FAILURES) {
                return fail(lastLegFailure);
            }
        } else {
            legFailures = 0;
        }
        if (queued != null) {
            leg = queued;
            queued = null;
            leg.enter(this, maid);
            idleTicks = 0;
        }
        return status;
    }

    /** 收手：停下、还重力，垫出来的方块由调用方决定拆不拆。 */
    public void cancel(EntityMaid maid) {
        if (leg != null) {
            leg.exit(this, maid);
            leg = null;
        }
        queued = null;
        restTicks = 0;
        MaidActuator.stopWalking(maid);
        status = Status.FAILED;
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

    private boolean satisfied(EntityMaid maid) {
        if (!(goal instanceof NavGoal.Stand target)) {
            return false;
        }
        BlockPos here = maid.blockPosition();
        // 站在目标格里不算到：那一格是要放东西的地方
        return !here.equals(target.target()) && target.canActFrom().test(here);
    }

    /** 没有腿在跑时重新规划：按由便宜到贵的顺序挑一种手段。 */
    private void plan(EntityMaid maid, SearchBudget budget) {
        phase = "规划";
        if (!(maid.level() instanceof ServerLevel level)) {
            fail(NavFailure.NO_STAND);
            return;
        }
        if (!maid.blockPosition().equals(memory.searchedFrom)) {
            memory.searchedFrom = maid.blockPosition().immutable();
            memory.unreachable.clear();
        }
        if (goal instanceof NavGoal.Settle) {
            planSettle(maid, level);
            return;
        }
        NavGoal.Stand target = (NavGoal.Stand) goal;
        BlockPos pos = target.target();
        BlockPos here = maid.blockPosition();

        // 目标明显在下面：先跳下去落到地面，再从地面找站位
        if (pos.getY() <= here.getY() - 2 && dropDown(maid, pos)) {
            return;
        }
        // 先找一个站得住、又能对它动手的位置走过去
        BlockPos found = StandSearch.forBuilding(
                maid, pos, policy.built(), memory.triedStands, memory.unreachable,
                target.canActFrom(), budget);
        if (found != null) {
            startWalk(maid, found);
            return;
        }
        // 走不过去：跑跳、平着搭路、垫脚、出屋，按这个顺序试
        if (hopTo(maid, target)) {
            return;
        }
        if (bridgeToward(maid, pos)) {
            return;
        }
        if (stepUp(maid, target)) {
            return;
        }
        // 站位搜索把这一 tick 的额度用完了：下个 tick 接着找，别急着判死
        if (budget.spent()) {
            return;
        }
        if (stageOutside(maid, pos, budget)) {
            return;
        }
        // 站在自己垫的方块上还是找不到路：先跳回地面重来
        if (book.standingOn(maid) && dropDown(maid, pos)) {
            return;
        }
        fail(canJumpHere(maid) ? NavFailure.NO_STAND : NavFailure.NO_HEADROOM);
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

    /** 目标下面两格以上：跳过缺口落到附近地面，再从地面重新找路。 */
    private boolean dropDown(EntityMaid maid, BlockPos target) {
        if (leg != null || !maid.onGround() || memory.drops >= 2) {
            return false;
        }
        Level level = maid.level();
        if (memory.hopUntil > level.getGameTime()) {
            return false;
        }
        Predicate<BlockPos> landing = spot -> spot.getY() <= target.getY() + 1
                && Math.abs(spot.getX() - target.getX()) <= 4
                && Math.abs(spot.getZ() - target.getZ()) <= 4
                && !book.contains(spot.below());
        List<BlockPos> chain = MaidParkour.planTo(
                maid, memory.triedStands, landing, ParkourRules.DEEP_FALL);
        if (chain.isEmpty()) {
            memory.hopUntil = level.getGameTime() + 40;
            return false;
        }
        stand = null;
        MaidParkour.Runner runner = new MaidParkour.Runner(maid, chain, ParkourRules.DEEP_FALL);
        if (runner.failed()) {
            memory.triedStands.addAll(chain);
            memory.hopUntil = level.getGameTime() + 40;
            return false;
        }
        memory.drops++;
        // 落到地面以后，从高处试出来的那批站位都不作数了，重新找
        memory.triedStands.clear();
        push(new HopLeg(runner, "跳下"), "跳过缺口落到地面");
        return true;
    }

    /** 缺口另一头够得着、又站得住：先试试跑跳过去。 */
    private boolean hopTo(EntityMaid maid, NavGoal.Stand target) {
        if (leg != null || !maid.onGround() || memory.hopUntil > maid.level().getGameTime()) {
            return false;
        }
        int maxFall = ParkourRules.MAX_FALL;
        List<BlockPos> chain = MaidParkour.planTo(
                maid, memory.triedStands, target.canActFrom(), maxFall);
        if (chain.isEmpty() && maid.getY() > target.target().getY() + ParkourRules.MAX_FALL) {
            // 高处够不到下面的站位：建筑模式免摔伤，直接从屋檐跳下去
            maxFall = ParkourRules.DEEP_FALL;
            chain = MaidParkour.planTo(maid, memory.triedStands, target.canActFrom(), maxFall);
        }
        if (chain.isEmpty()) {
            memory.hopUntil = maid.level().getGameTime() + 40;
            return false;
        }
        stand = null;
        MaidParkour.Runner runner = new MaidParkour.Runner(maid, chain, maxFall);
        if (runner.failed()) {
            memory.triedStands.addAll(chain);
            memory.hopUntil = maid.level().getGameTime() + 40;
            return false;
        }
        push(new HopLeg(runner, "跑跳过缺口"), "跑跳过缺口");
        return true;
    }

    /**
     * 平着搭路：目标与脚下齐平、中间又走不过去时，就地铺一格、走过去，再接着铺。
     *
     * <p>前面一格本来就站得住就直接走过去，不多放一块。
     */
    private boolean bridgeToward(EntityMaid maid, BlockPos target) {
        BlockPos feet = maid.blockPosition();
        int dx = target.getX() - feet.getX();
        int dz = target.getZ() - feet.getZ();
        if (!ScaffoldRules.mayBridge(target.getY() - feet.getY())
                || Math.abs(dx) + Math.abs(dz) < 2) {
            return false;
        }
        if (memory.bridges >= ScaffoldRules.MAX_BRIDGE
                || !ScaffoldRules.nearEnough(dx, dz)
                || book.size() >= ScaffoldRules.MAX_BLOCKS
                || !canJumpHere(maid)) {
            return false;
        }
        Direction step = ScaffoldRules.alongX(dx, dz)
                ? Direction.getNearest(dx, 0, 0)
                : Direction.getNearest(0, 0, dz);
        BlockPos ahead = feet.relative(step);
        // 前面是目标格或图纸要的位置就不能站进去：那是要放方块的地方
        if (ahead.equals(target) || !policy.canBuildOn(ahead)) {
            return false;
        }
        if (memory.triedStands.contains(ahead)) {
            return false;
        }
        if (!Terrain.free(maid.level(), ahead)
                || !Terrain.free(maid.level(), ahead.above())
                || Bodies.self(ahead, maid)
                || Bodies.other(maid.level(), ahead, maid)) {
            return false;
        }
        if (Terrain.solid(maid.level(), ahead.below())) {
            startWalk(maid, ahead);
            return true;
        }
        // 前面悬空：在它下面铺一块，走过去还是同一个高度
        BlockPos floor = ahead.below();
        if (!policy.canBuildOn(floor)
                || !Terrain.free(maid.level(), floor)
                || !Terrain.reachable(maid, floor)
                || Bodies.other(maid.level(), floor, maid)) {
            return false;
        }
        Item item = ScaffoldBook.item(maid, policy.consumeMaterials());
        if (item == null
                || !book.put((ServerLevel) maid.level(), maid, floor, item,
                        policy.consumeMaterials())) {
            return false;
        }
        memory.bridges++;
        startWalk(maid, ahead);
        return true;
    }

    /**
     * 够不着时在她脚下垫一格：升上去、或者平着挪到能放的位置。
     *
     * <p>托底那块先垫：顶块立在一块空地上放不住，得有下面的方块给它当依托。
     * 四面没有能垫的位置时改原地起跳。
     */
    private boolean stepUp(EntityMaid maid, NavGoal.Stand target) {
        if (memory.steps >= ScaffoldRules.MAX_STEPS
                || book.size() >= ScaffoldRules.MAX_BLOCKS) {
            return false;
        }
        BlockPos feet = maid.blockPosition();
        BlockPos pos = target.target();
        int rise = pos.getY() - feet.getY();
        if (!ScaffoldRules.mayStep(rise)) {
            return false;
        }
        // 头顶被方块压住时她既跳不起来也迈不上去：先走开，别在原地耗
        if (!canJumpHere(maid)) {
            return false;
        }
        if (!ScaffoldRules.nearEnough(pos.getX() - feet.getX(), pos.getZ() - feet.getZ())) {
            return false;
        }
        Item item = ScaffoldBook.item(maid, policy.consumeMaterials());
        if (item == null) {
            return false;
        }
        Vec3 goalCenter = Vec3.atCenterOf(pos);
        double current = Vec3.atBottomCenterOf(feet).distanceToSqr(goalCenter);
        // 目标就在正上方：原地跳起来把脚下垫高
        if (rise >= 2 && pos.getX() == feet.getX() && pos.getZ() == feet.getZ()
                && jumpUp(maid, feet, item)) {
            return true;
        }
        BlockPos bestTop = null;
        boolean bestSupportReady = false;
        double bestCost = Double.MAX_VALUE;
        BlockPos placeTop = null;
        boolean placeSupportReady = false;
        double placeCost = Double.MAX_VALUE;
        // 只往上下左右四个方向垫：斜着放她会从斜角起跳，被旁边的方块一挡就落回原高度
        for (Direction side : Direction.Plane.HORIZONTAL) {
            BlockPos top = feet.relative(side);
            if (!stepRoom(maid, top)) {
                continue;
            }
            double score = Vec3.atBottomCenterOf(top.above()).distanceToSqr(goalCenter);
            if (ScaffoldRules.mustApproach(rise) && score > current - 0.5) {
                continue;
            }
            BlockPos support = top.below();
            boolean supportReady = Terrain.solid(maid.level(), support);
            if (!supportReady) {
                if (!policy.canBuildOn(support) || !Terrain.free(maid.level(), support)
                        || Placement.context(maid, new ItemStack(item), support, null, null, null)
                                == null
                        || !Placement.seesNow(maid, support, null)) {
                    continue;
                }
            } else if (!Placement.seesNow(maid, top, null)) {
                // 垫脚块也得是她看得见的地方：看不见就换一侧
                continue;
            }
            // 底下已经实心就省下一块料，同分时优先
            double cost = score + (supportReady ? 0.0 : 0.5);
            if (target.canActFrom().test(top.above()) && cost < placeCost) {
                placeCost = cost;
                placeTop = top;
                placeSupportReady = supportReady;
            }
            if (cost < bestCost) {
                bestCost = cost;
                bestTop = top;
                bestSupportReady = supportReady;
            }
        }
        // 垫完就能动手的站位优先：目标与她齐平时，这一格就是为了站过去
        if (placeTop != null) {
            bestTop = placeTop;
            bestSupportReady = placeSupportReady;
        }
        if (bestTop == null) {
            return jumpUp(maid, feet, item);
        }
        ServerLevel level = (ServerLevel) maid.level();
        if (!bestSupportReady) {
            if (!book.put(level, maid, bestTop.below(), item, policy.consumeMaterials())) {
                return false;
            }
            // 托底只是前半个动作，升高数等到顶块放上去才算
            queued = new StepLeg(bestTop, item);
            phase = "垫脚：放托底";
            return true;
        }
        if (!book.put(level, maid, bestTop, item, policy.consumeMaterials())) {
            return false;
        }
        memory.steps++;
        startWalk(maid, bestTop.above());
        return true;
    }

    /** 原地跳起来把脚下的方块补上，落下来就升了一整格。 */
    private boolean jumpUp(EntityMaid maid, BlockPos feet, Item item) {
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

    /** 在屋里被天花板顶住时，先去附近一处露天的地方，再沿外墙往上垫。 */
    private boolean stageOutside(EntityMaid maid, BlockPos target, SearchBudget budget) {
        if (maid.level().canSeeSky(maid.blockPosition().above())) {
            return false;
        }
        BlockPos stage = StandSearch.stageFor(maid, target, budget);
        if (stage == null) {
            return false;
        }
        startWalk(maid, stage);
        return true;
    }

    /** 她此刻能不能起跳：脚与头两格之外，头顶上面还要再让出一格。 */
    private boolean canJumpHere(EntityMaid maid) {
        BlockPos feet = maid.blockPosition();
        return Terrain.free(maid.level(), feet.above())
                && Terrain.free(maid.level(), feet.above(2));
    }

    /** 这一格能不能当垫脚石：位置空着、头顶有地方站、不在图纸里、她伸手够得着。 */
    private boolean stepRoom(EntityMaid maid, BlockPos top) {
        if (!policy.canBuildOn(top) || !Terrain.free(maid.level(), top)) {
            return false;
        }
        // 站上去要两格高：脚一格、头一格，否则会被房顶卡住
        if (!Terrain.free(maid.level(), top.above())
                || !Terrain.free(maid.level(), top.above(2))) {
            return false;
        }
        return !Bodies.self(top, maid)
                && !Bodies.other(maid.level(), top, maid)
                && !Bodies.other(maid.level(), top.above(), maid)
                && Terrain.reachable(maid, top);
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
                memory.hopUntil = maid.level().getGameTime() + 40;
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

    /** 垫脚：托底已经放好，把顶块放上再走上去。 */
    private final class StepLeg implements Leg {
        private final BlockPos top;
        private final Item item;

        private StepLeg(BlockPos top, Item item) {
            this.top = top;
            this.item = item;
        }

        @Override
        public Outcome tick(Navigator nav, EntityMaid maid) {
            if (memory.steps >= ScaffoldRules.MAX_STEPS
                    || !stepRoom(maid, top)
                    || !book.put((ServerLevel) maid.level(), maid, top, item,
                            policy.consumeMaterials())) {
                lastLegFailure = NavFailure.NO_STAND;
                return Outcome.FAILED;
            }
            memory.steps++;
            queued = new WalkLeg(top.above());
            phase = "垫脚后走上新格";
            return Outcome.DONE;
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
