package com.lingchat.maidbridge.action.construction;

import com.github.tartaricacid.touhoulittlemaid.api.task.IMaidTask;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.github.tartaricacid.touhoulittlemaid.entity.task.TaskIdle;
import com.github.tartaricacid.touhoulittlemaid.entity.task.TaskManager;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.action.construction.BuildRules.CellState;
import com.lingchat.maidbridge.action.construction.BuildRules.Failure;
import com.lingchat.maidbridge.action.construction.BuildRules.PassStats;
import com.lingchat.maidbridge.action.WanderGuard;
import com.lingchat.maidbridge.action.core.MaidActionController;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.action.nav.NavGoal;
import com.lingchat.maidbridge.action.nav.NavMemory;
import com.lingchat.maidbridge.action.nav.NavPolicy;
import com.lingchat.maidbridge.action.nav.Navigator;
import com.lingchat.maidbridge.action.nav.ScaffoldBook;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.debug.PathDebug;
import com.lingchat.maidbridge.debug.TestFeedback;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.EventFrames;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.state.BlockState;

import static com.lingchat.maidbridge.action.construction.BuildLayer.*;
import static com.lingchat.maidbridge.action.construction.BuildPlanner.*;

/**
 * 按图纸盖房子：扫描决定这一格怎么处理，寻路负责把她送到能干活的位置，放完歇一下再扫。
 *
 * <p>与挖掘、放置一样是单槽位、每 tick 推进的异步活。施工顺序是先低层后高层，
 * 同一层里先放脚下有依托的格子；一遍放不完就再走一遍，让新放下的方块给邻居当依托。
 * 单格判据只看这一格此刻“是什么、够不够得着、放不放得下”，不经过 LLM。
 *
 * <p>走动、跳下、跑跳、垫脚、搭路全部由寻路模块负责；这里只给目标「站到能放下这一格的
 * 位置」和边界「图纸格不许垫」。失败原因由寻路回报，这里翻成格子账上的原因。
 *
 * <p>生存模式从她背包里扣方块；主人是创造模式时不扣料，方便测试。缺料不硬等：
 * 这一格记成 short_of_material，别的能放继续放，收尾时把还差多少件一起交回插件。
 */
public final class MaidBuildController {
    static final org.slf4j.Logger LOGGER =
            org.slf4j.LoggerFactory.getLogger("lingchat_maid_bridge");
    /** 每放一块之间的基础间隔，再叠一点随机，避免像 for 循环一样一块接一块。 */
    static final int PLACE_INTERVAL_BASE = 5;
    static final int PLACE_INTERVAL_JITTER = 4;
    /** 下一格要转大角度时额外多等几 tick，让视线先转过去。 */
    static final int TURN_EXTRA_TICKS = 5;
    /** 两遍之间歇一下。 */
    static final int PASS_BREAK_TICKS = 2;
    /** 从受理到收工最多等这么久（约四十分钟）：中型房子的块数多，二十分钟会半途收手。 */
    static final int BUILD_TIMEOUT_TICKS = 48000;
    /** 一 tick 最多判几格，避免第一次扫描时对整张图纸算寻路。 */
    static final int MAX_SCAN_PER_TICK = 6;
    /** 目标格被活物占着时先放一放，过这么久再试。 */
    static final int ENTITY_RETRY_TICKS = 100;
    /** 这一层全被活物占着时最多等这么久（约十秒）；再等下去不如先搁置换层。 */
    static final int HOLD_GIVEUP_TICKS = 200;
    /** 同一格连续因为寻路没推进而停住，最多等这么久（约六秒）。 */
    static final int MAX_CELL_WAITS = 120;
    /** 调试时多久算一次「下一块放哪」：每格都要验视线，不能每 tick 都算。 */
    static final int NEXT_PLACE_TICKS = 10;
    /** 收尾时走去一个落脚点、拆净垫脚，最多等这么久。 */
    static final int CLEANUP_TIMEOUT_TICKS = 120;

    /** 图纸里的一格，已经换算到世界坐标。 */
    public record Target(BlockPos pos, Block block, Float yaw) {
    }


    static final class Cell {
        final BlockPos pos;
        final Block block;
        final Item item;
        final String itemId;
        final Float yaw;
        /** 这一格在整轮施工里的进度：待做、搁置等补试、补试中、完成、失败。 */
        CellState state = CellState.PENDING;
        /** 最后一次没放上的原因，搁置或收账时按它记账。 */
        Failure failure = Failure.NO_STAND;
        /** 这一格自己的寻路记忆：试过哪些站位、垫过几次脚、跳了几次。 */
        final NavMemory nav = new NavMemory();
        /** 已经为这一格在目标底下垫过依托，再没有依托就真放不下了。 */
        boolean propped;
        /** 这一格被活物占着，等到这个游戏刻之后再试。 */
        long holdUntil;
        /** 连续因为寻路没推进而停在这一格的 tick 数。 */
        int waits;
        /** 为这一格开过几次寻路都失败了：连试两次还不行就先搁到补试轮。 */
        int navFails;
        /** 有没有真的为它试过：没试过的格子收账时不拿默认原因顶数。 */
        boolean attempted;

        Cell(Target target) {
            this.pos = target.pos();
            this.block = target.block();
            this.item = target.block().asItem();
            this.itemId = BuiltInRegistries.ITEM.getKey(this.item).toString();
            this.yaw = target.yaw();
        }
    }

    static final class Run {
        final String maidId;
        final ServerLevel level;
        final String name;
        final boolean consumeMaterials;
        final List<Cell> cells;
        final int total;
        final Map<String, Integer> missing = new LinkedHashMap<>();
        int placed;
        int ticks;
        int cooldown;
        /** 这一 tick 走哪个相位；由运行状态推导，切换时跑进入与退出钩子。 */
        BuildPhase phase = BuildPhase.SCAN;
        /** 正在跑的寻路；为空表示不在赶路。 */
        Navigator nav;
        /** 这次寻路是为哪一格开的。 */
        Cell navCell;
        /** 她正赶过去或正踩着的落脚点；选格和收尾时用来护着那一格。 */
        BlockPos stand;
        /** 为哪一格选了落脚点：她没到之前，脚下这一格先别放方块堵死。 */
        Cell standFor;
        /** 这一轮施工垫下的临时方块。 */
        final ScaffoldBook book = new ScaffoldBook();
        /** 本层收尾时冻结的统计。 */
        PassStats passStats;
        /** 当前处理的层在格子表里的下标区间。 */
        int layerStart;
        int layerEnd;
        int cursor;
        int layerPassPlaced;
        int stallPasses;
        /** 这一层第一次全被活物占住的时间；挪开或换层就清零。 */
        long heldSince;
        /** 图纸里已经放好的格子，用来优先站到自己盖出来的面上。 */
        final Set<BlockPos> built = new HashSet<>();
        /** 图纸占着的格子：垫脚不能占掉图纸自己的位置。 */
        final Set<BlockPos> blueprint = new HashSet<>();
        /** 已经做过一轮补试：剩下的格子再不行就真有定论了。 */
        boolean retried;
        /** 正在处理的那一格：换了站位、退了再试时都先盯着它，别扫到别的格子去。 */
        Cell active;
        /** 这一层的顺序该按她此刻的位置重排了：换层、重开一遍时各排一次。 */
        boolean needOrder;
        /** 正在收尾：先离开垫脚块、把它拆干净，再推收尾。 */
        boolean finishing;
        int doneTicks;
        String doneCode = "";
        String doneReason = "";
        /** 收尾时按原因记账。 */
        final Map<String, Integer> failed = new LinkedHashMap<>();
        /** 施工前她的工作模式：收手时换回去，别让她盖完房子还一直闲站着。 */
        IMaidTask previousTask;
        /** 调试用：下一块大概率要放的位置，隔一会儿重算一次。 */
        BlockPos nextPlace;
        int nextPlaceIn;
        /** 调试计数：有多少次因为这一格找不到依托面而跳过。 */
        int noSupportSkips;
        /** 调试计数：有多少次撞上挖不动的方块。 */
        int blockedSkips;

        Run(EntityMaid maid, String name, List<Cell> cells, boolean consumeMaterials) {
            this.maidId = maid.getStringUUID();
            this.level = (ServerLevel) maid.level();
            this.name = name;
            this.cells = cells;
            this.total = cells.size();
            this.consumeMaterials = consumeMaterials;
        }
    }


    static Run current;
    /** 这一 tick 从哪个出口结束的：连续停在同一个出口就说明在原地打转。 */
    static String lastStop = "";
    static int stopTicks;

    /** 记一笔本 tick 的出口；同一出口连续出现时累加，状态栏里一眼能看出循环。 */
    static void stop(String where) {
        if (where.equals(lastStop)) {
            stopTicks++;
        } else {
            lastStop = where;
            stopTicks = 1;
        }
    }

    /** 上一张图纸的锚点与状态：补料后重调时用它接着原来的工地，而不是按新朝向另起一座。 */
    static String lastSignature = "";
    static BlockPos lastAnchor;
    static int lastBaseY;
    static ServerLevel lastLevel;
    static String lastDimension = "";
    static String lastStatus = "";
    static String lastReasonCode = "";
    static String lastReason = "";
    static String lastName = "";
    static int lastPlaced;
    static int lastTotal;
    /** 上一张图纸已经换算好的世界坐标；接着盖时原样复用，免得旋转后的图纸和半成品对不上。 */
    static List<Target> lastTargets = List.of();

    MaidBuildController() {
    }

    /** 当前或上一座房子的施工进度；没盖过返回 status=none。 */
    public static JsonObject status() {
        JsonObject data = new JsonObject();
        Run run = current;
        if (run != null) {
            data.addProperty("status", "running");
            data.addProperty("name", run.name);
            data.addProperty("placed", run.placed);
            data.addProperty("total", run.total);
            data.addProperty("remaining", Math.max(0, run.total - run.placed));
            return data;
        }
        if (lastAnchor == null) {
            data.addProperty("status", "none");
            data.addProperty("name", "");
            data.addProperty("placed", 0);
            data.addProperty("total", 0);
            data.addProperty("remaining", 0);
            data.addProperty("reason_code", "");
            data.addProperty("reason", "");
            return data;
        }
        data.addProperty("status", lastStatus.isEmpty() ? "unknown" : lastStatus);
        data.addProperty("name", lastName);
        data.addProperty("placed", lastPlaced);
        data.addProperty("total", lastTotal);
        data.addProperty("remaining", Math.max(0, lastTotal - lastPlaced));
        data.addProperty("reason_code", lastReasonCode);
        data.addProperty("reason", lastReason);
        return data;
    }

    /** 接着上一处工地需要的锚点、地板高度与已经算好的格子。 */
    public record Resume(BlockPos anchor, int baseY, List<Target> targets) {
    }

    /**
     * 上一张同签名图纸还没盖完时，返回原工地；否则返回 null。
     *
     * <p>只看图纸签名与这一局的维度实例，不看她现在离得多远、朝哪边。她跑去别处之后再
     * 调一次，应该走回原来的工地接着盖，而不是按当前站位另起一座。工地区块没加载时由
     * handler 逐格核出来并拒绝，不会悄悄退化成新工地；换了存档或重开世界时维度实例
     * 对不上，也不会把上一局的工地当成这一局的。
     */
    public static Resume resume(String signature, EntityMaid maid) {
        if (lastAnchor == null || "done".equals(lastStatus)
                || !lastSignature.equals(signature) || lastTargets.isEmpty()) {
            return null;
        }
        String dimension = maid.level().dimension().location().toString();
        if (lastLevel == null || maid.level() != lastLevel || !lastDimension.equals(dimension)) {
            return null;
        }
        return new Resume(lastAnchor, lastBaseY, lastTargets);
    }

    /** 受理一张图纸：格子已经由 handler 核过，这里只排序并清掉没做完的旧活。 */
    public static void start(
            MinecraftServer server, EntityMaid maid, String name, String signature,
            BlockPos anchor, int baseY, List<Target> targets, boolean consumeMaterials) {
        cancel(server);
        // 新工地接手前，别的会写寻路的也一并收掉
        MaidActionController.stopOtherActions(server, maid, "被施工的事叫走了");
        MaidActuator.beginWalk(maid);
        // 施工这十几分钟里按住她自己的随机游荡与捡东西：挖、放、盖都一样
        WanderGuard.begin(maid);

        lastSignature = signature;
        lastAnchor = anchor;
        lastBaseY = baseY;
        lastLevel = (ServerLevel) maid.level();
        lastDimension = maid.level().dimension().location().toString();
        lastStatus = "";
        lastReasonCode = "";
        lastReason = "";
        lastName = name;
        lastPlaced = 0;
        lastTotal = targets.size();
        lastTargets = List.copyOf(targets);

        List<Cell> cells = new ArrayList<>(targets.size());
        for (Target target : targets) {
            cells.add(new Cell(target));
        }
        BlockPos here = maid.blockPosition();
        cells.sort(Comparator.<Cell>comparingInt(cell -> cell.pos.getY())
                .thenComparingInt(cell -> supported((ServerLevel) maid.level(), cell.pos) ? 0 : 1)
                .thenComparingDouble(cell -> cell.pos.distSqr(here)));

        Run run = new Run(maid, name, cells, consumeMaterials);
        // 干活模式下她有自己的寻路（农场、钓鱼……），与施工抢同一份目标：先切成空闲，
        // 盖完再换回去。换任务会重建 brain，正好把她那一刻残留的目标一起清掉
        IMaidTask task = maid.getTask();
        if (task != null && !TaskIdle.UID.equals(task.getUid())) {
            run.previousTask = task;
            maid.setTask(TaskManager.getIdleTask());
        }
        for (Cell cell : cells) {
            run.blueprint.add(cell.pos);
            BlockState state = run.level.getBlockState(cell.pos);
            if (matches(cell, state)) {
                CellFlow.done(run, cell);
            } else if (state.getBlock() instanceof LiquidBlock) {
                CellFlow.fail(run, cell, Failure.LIQUID);
            }
            // 被别的方块占着的先不判死：施工时会走过去把它挖掉，再放图纸这一块
        }
        current = run;
        if (run.placed >= run.total || !resetLayer(run)) {
            finish(server, run, "");
        }
    }

    /** 这只女仆正在被我带着盖：跟随行为给长任务让路时用。 */
    public static boolean isRunning(EntityMaid maid) {
        Run run = current;
        return run != null && maid != null && run.maidId.equals(maid.getStringUUID());
    }

    /** 被别的指令顶掉、停服时收手；不广播，新的指令会自己交代。 */
    public static void cancel(MinecraftServer server) {
        Run run = current;
        current = null;
        lastSignature = "";
        lastAnchor = null;
        lastBaseY = 0;
        lastLevel = null;
        lastDimension = "";
        lastStatus = "";
        lastReasonCode = "";
        lastReason = "";
        lastName = "";
        lastPlaced = 0;
        lastTotal = 0;
        lastTargets = List.of();
        if (run == null) {
            return;
        }
        EntityMaid maid = MaidQueries.findMaidById(server, run.maidId);
        if (maid != null) {
            // 换工地、停服时不讲究落脚点，垫出来的方块一块都不留
            stopNav(run, maid);
            run.book.clearNow(run.level, maid, run.consumeMaterials);
            MaidActuator.stopWalking(maid);
            MaidActuator.stopLooking(maid);
            restoreTask(run, maid);
        }
        WanderGuard.end(run.maidId);
    }

    /** 把她施工前的工作模式换回去。 */
    static void restoreTask(Run run, EntityMaid maid) {
        if (run.previousTask != null) {
            maid.setTask(run.previousTask);
            run.previousTask = null;
        }
    }

    /** 被别的指令顶掉：收手并给插件一个收尾，别让她一直以为还在盖。 */
    public static void cancelByOther(MinecraftServer server, String reason) {
        Run run = current;
        if (run == null) {
            return;
        }
        finish(server, run, "cancelled", reason);
    }

    /** 昏迷打断：收手并给插件一个收尾。 */
    public static void cancelForUnconscious(MinecraftServer server) {
        Run run = current;
        if (run == null) {
            return;
        }
        finish(server, run, "unconscious", "她昏过去了");
    }

    /** 每个服务端 tick 推进一次（主线程）。 */
    public static void tick(MinecraftServer server) {
        Run run = current;
        if (run == null) {
            return;
        }
        EntityMaid maid = MaidQueries.findMaidById(server, run.maidId);
        if (maid == null || !maid.isAlive() || maid.level() != run.level) {
            // 她没了（死在别处、区块卸载）：死亡与受伤旁白会说明，不另发施工收尾
            current = null;
            return;
        }
        if (MaidQueries.carriedByPlayer(maid)) {
            finish(server, run, "carried", "玩家把她抱起来了");
            return;
        }
        if (maid.isSleeping() || maid.isMaidInSittingPose() || maid.isPassenger()) {
            MaidActuator.standUp(maid);
        }
        if (++run.ticks > BUILD_TIMEOUT_TICKS) {
            finish(server, run, "timeout", "盖了这么久还没盖完，先停下了");
            return;
        }
        if (run.nextPlaceIn > 0) {
            run.nextPlaceIn--;
        }
        // 调试可视化开着时，把这一 tick 的阶段、落脚点与目标交上去；没开就只多一次空判断
        if (PathDebug.watching(maid)) {
            reportDebug(run, maid);
        }
        // 没有寻路在跑时，顺手把走开的垫脚块拆掉；她正赶去的落脚点附近先护着
        if (run.nav == null) {
            run.book.clean(run.level, maid, run.stand, false, false, run.consumeMaterials);
        }
        BuildPhase.to(server, run, maid, BuildPhase.of(run));
        run.phase.tick(server, run, maid);
    }

    /** 扫描相位：本层按位置重排后取一格处理；扫完就数账、按决策表走。 */
    static void scan(MinecraftServer server, Run run, EntityMaid maid) {
        if (run.needOrder) {
            run.needOrder = false;
            orderLayer(run, maid);
        }
        if (next(run, maid)) {
            return;
        }
        // 这一 tick 的扫描额度用完、本层还有格子没看过：下个 tick 接着扫，别当成整层扫完
        if (run.cursor < run.layerEnd) {
            return;
        }
        endPass(server, run, measurePass(run));
    }

    /** 赶路相位：寻路自己挑手段；到了就试放，走不通就记账回去扫。 */
    static void pathing(MinecraftServer server, Run run, EntityMaid maid) {
        Navigator nav = run.nav;
        if (nav == null) {
            return;
        }
        Navigator.Status status = nav.tick(maid);
        nav.cleanScaffold(maid);
        run.stand = nav.stand();
        if (status == Navigator.Status.RUNNING) {
            return;
        }
        Cell cell = run.navCell;
        run.nav = null;
        run.navCell = null;
        run.standFor = null;
        if (status == Navigator.Status.ARRIVED) {
            if (cell != null) {
                cell.navFails = 0;
            }
            arrive(run, maid);
            return;
        }
        run.stand = null;
        if (cell != null) {
            Failure cause = failureOf(nav.failure());
            if (++cell.navFails >= 2) {
                CellFlow.park(run, cell, cause);
            } else {
                CellFlow.holdThenPark(run, cell, cause);
            }
        }
        run.cooldown = PASS_BREAK_TICKS;
    }

    /** 放置间隔：放完一块或一遍走完，歇几个 tick 再扫。 */
    static void cooldown(Run run) {
        if (run.cooldown > 0) {
            run.cooldown--;
        }
    }

    /** 收尾相位：先走去实地上站好、拆净垫脚，再推收尾事件；等太久就强拆。 */
    static void settling(MinecraftServer server, Run run, EntityMaid maid) {
        run.doneTicks++;
        if (run.nav != null) {
            run.nav.tick(maid);
            run.nav.cleanScaffold(maid);
            run.stand = run.nav.stand();
            if (run.nav.status() == Navigator.Status.RUNNING
                    && run.doneTicks < CLEANUP_TIMEOUT_TICKS) {
                return;
            }
            run.nav = null;
        }
        run.book.clearNow(run.level, maid, run.consumeMaterials);
        finishNow(server, run, run.doneCode, run.doneReason);
    }

    /** 收手时把寻路停掉，别让她还在往半空里的落脚点走。 */
    static void stopNav(Run run, EntityMaid maid) {
        if (run.nav != null) {
            run.nav.cancel(maid);
            run.nav = null;
        }
        run.navCell = null;
        run.standFor = null;
        run.stand = null;
    }

    /**
     * 调试可视化：这一 tick 她在干什么、正对着哪一格、站在哪、下一块大概放哪里。
     */
    static void reportDebug(Run run, EntityMaid maid) {
        predictNext(run, maid);
        Cell focus = run.active != null && run.active.state.open() ? run.active : null;
        String phase;
        if (run.finishing) {
            phase = "收尾：收拾垫脚块";
        } else if (run.nav != null) {
            phase = run.nav.phase();
        } else if (run.cooldown > 0) {
            phase = "放置间隔";
        } else if (focus != null) {
            phase = "准备放这一格";
        } else {
            phase = "扫描下一格";
        }
        BlockPos target = focus != null ? focus.pos : run.nextPlace;
        Cell named = focus != null ? focus : cellAt(run, run.nextPlace);
        String block = named == null
                ? ""
                : BuiltInRegistries.BLOCK.getKey(named.block).toString();
        PathDebug.report(
                maid, phase, run.stand, target,
                run.nav == null ? List.of() : run.nav.chain(),
                run.nextPlace, block);
    }

    /** 按坐标回头找图纸里那一格：调试栏里报方块名用。 */
    static Cell cellAt(Run run, BlockPos pos) {
        if (pos == null) {
            return null;
        }
        for (Cell cell : run.cells) {
            if (cell.pos.equals(pos)) {
                return cell;
            }
        }
        return null;
    }

    /** 隔一会儿算一次「下一块放哪」：放置、开走之后立即作废，下一次用到时重算。 */
    static BlockPos predictNext(Run run, EntityMaid maid) {
        if (run.nextPlaceIn <= 0) {
            run.nextPlace = nextPlacement(run, maid);
            run.nextPlaceIn = NEXT_PLACE_TICKS;
        }
        return run.nextPlace;
    }


    /** 调试指令里补一句施工进度：这一层还剩什么、上一 tick 的寻路额度剩多少。 */
    public static String debugStatus() {
        Run run = current;
        if (run == null) {
            return "｜施工：没有在进行";
        }
        boolean doorsOnly = onlyDoorsLeft(run);
        int open = 0;
        int waiting = 0;
        int reserved = 0;
        for (int i = run.layerStart; i < run.layerEnd; i++) {
            Cell cell = run.cells.get(i);
            if (!inPass(cell, doorsOnly)) {
                continue;
            }
            open++;
            if (held(cell, run.level)) {
                waiting++;
            }
            if (reservedStand(run, cell)) {
                reserved++;
            }
        }
        long waited = run.heldSince == 0
                ? 0
                : Math.max(0, run.level.getGameTime() - run.heldSince) / 20;
        int pending = 0;
        int retrying = 0;
        int failed = 0;
        int deferred = 0;
        for (Cell cell : run.cells) {
            switch (cell.state) {
                case PENDING -> pending++;
                case RETRYING -> retrying++;
                case FAILED -> failed++;
                case DEFERRED -> deferred++;
            }
        }
        String hand = run.active == null ? "-" : run.active.pos + ":" + run.active.state;
        String walking = run.nav == null ? "-" : run.nav.phase();
        return "｜施工 " + run.placed + "/" + run.total
                + "｜相 " + BuildPhase.of(run)
                + "｜步 " + walking
                + "｜本层 开 " + open + "·等活物 " + waiting + "·预留 " + reserved
                + (waited > 0 ? "·已等 " + waited + "s" : "")
                + "｜全 待 " + pending + "·补 " + retrying + "·败 " + failed
                + "·搁置 " + deferred
                + "｜停遍 " + run.stallPasses + (run.retried ? "·已补" : "")
                + "｜停 " + lastStop + "×" + stopTicks
                + "｜行 " + run.cursor + "/" + run.layerStart + "-" + run.layerEnd
                + "｜手 " + hand
                + "｜垫 " + run.book.size()
                + "｜心跳 " + run.ticks;
    }


    static void finish(MinecraftServer server, Run run, String reasonCode) {
        finish(server, run, reasonCode, "");
    }

    /**
     * 收工：还有垫脚块时先走开、拆干净再推收尾事件，别让她停在临时方块上。
     *
     * <p>被人抱走、打晕、被别的指令顶掉时不拖：那几种情况她本来就要离开，
     * 直接按下面的收尾走，能拆的当场拆掉。
     */
    static void finish(MinecraftServer server, Run run, String reasonCode, String reason) {
        EntityMaid maid = MaidQueries.findMaidById(server, run.maidId);
        boolean natural = reasonCode == null || reasonCode.isEmpty() || "timeout".equals(reasonCode);
        if (natural && !run.finishing && maid != null && !maid.isRemoved()
                && !run.book.isEmpty()) {
            run.finishing = true;
            run.doneCode = reasonCode == null ? "" : reasonCode;
            run.doneReason = reason == null ? "" : reason;
            run.doneTicks = 0;
            stopNav(run, maid);
            run.nav = Navigator.start(
                    new NavGoal.Settle(),
                    new NavPolicy(run.blueprint, run.built, run.consumeMaterials),
                    new NavMemory(), run.book);
            return;
        }
        finishNow(server, run, reasonCode, reason);
    }

    static void finishNow(MinecraftServer server, Run run, String reasonCode, String reason) {
        EntityMaid maid = MaidQueries.findMaidById(server, run.maidId);
        if (maid != null) {
            // 中断的路上不再护着脚下那块：垫出来的方块一块都不留
            stopNav(run, maid);
            run.book.clearNow(run.level, maid, run.consumeMaterials);
            MaidActuator.stopWalking(maid);
            MaidActuator.stopLooking(maid);
            restoreTask(run, maid);
        }
        current = null;
        WanderGuard.end(run.maidId);

        // 补试阶段又落下的格子也要有个说法，别在失败清单里凭空消失
        for (Cell cell : run.cells) {
            if (cell.state == CellState.DEFERRED) {
                CellFlow.fail(run, cell, cell.failure);
            }
        }
        int remaining = Math.max(0, run.total - run.placed);
        String status = remaining == 0 ? "done" : (run.placed > 0 ? "partial" : "failed");
        lastStatus = status;
        lastPlaced = run.placed;
        lastTotal = run.total;
        String code = reasonCode == null ? "" : reasonCode;
        if (status.equals("done")) {
            code = "";
        } else if (code.isEmpty()) {
            code = naturalReason(run);
        }
        String text = reason == null || reason.isEmpty() ? reasonText(code) : reason;
        lastReasonCode = status.equals("done") ? "" : code;
        lastReason = status.equals("done") ? "" : text;
        // 调试指令等着的施工回执：盖完、盖了一半、没盖成都在这里交代
        TestFeedback.buildDone(
                server, run.maidId, status, code, text, run.placed, run.total, run.ticks);
        LOGGER.info(
                "施工收工：{}，放置 {}/{}，原因 {}，失败 {}，无支撑跳过 {}，挖不动 {}",
                status, run.placed, run.total, code, run.failed,
                run.noSupportSkips, run.blockedSkips);

        JsonObject data = EventFrames.base("maid_build_done", run.maidId);
        data.addProperty("status", status);
        data.addProperty("name", run.name);
        data.addProperty("placed", run.placed);
        data.addProperty("total", run.total);
        data.addProperty("remaining", remaining);
        data.addProperty("reason_code", code);
        data.addProperty("reason", status.equals("done") ? "" : text);
        JsonArray materials = new JsonArray();
        for (Map.Entry<String, Integer> entry : run.missing.entrySet()) {
            JsonObject one = new JsonObject();
            one.addProperty("item", entry.getKey());
            one.addProperty("count", entry.getValue());
            materials.add(one);
        }
        data.add("materials", materials);

        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge != null && bridge.hasClients()) {
            bridge.broadcastEvent(data);
        }
    }

    static String naturalReason(Run run) {
        if (!run.missing.isEmpty()) {
            return "short_of_material";
        }
        for (String code : List.of("no_path", "no_stand", "no_headroom", "blocked_by_entity",
                "self_blocked", "place_failed", "blocked", "liquid", "out_of_reach")) {
            if (run.failed.containsKey(code)) {
                return code;
            }
        }
        return "incomplete";
    }

    static String reasonText(String code) {
        return switch (code == null ? "" : code) {
            case "short_of_material" -> "材料不够了";
            case "out_of_reach" -> "有几格她够不着或者放不下";
            case "no_path" -> "有几格她走不过去";
            case "no_stand" -> "有几格旁边没有能站的地方";
            case "no_headroom" -> "有几格她待的地方头顶太低，跳不起来";
            case "blocked_by_entity" -> "有几格被活物占着";
            case "self_blocked" -> "有几格她自己挡着，站不开";
            case "place_failed" -> "有几格试了还是没放上";
            case "blocked" -> "有几格被别的东西占着";
            case "liquid" -> "有几格是液体，放不了";
            case "timeout" -> "盖得太久，先停下了";
            case "cancelled" -> "被别的事叫走了";
            case "unconscious" -> "她昏过去了";
            case "carried" -> "玩家把她抱起来了";
            default -> "还没盖完";
        };
    }
}
