package com.lingchat.maidbridge.action.construction;

import com.github.tartaricacid.touhoulittlemaid.api.task.IMaidTask;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.github.tartaricacid.touhoulittlemaid.entity.task.TaskIdle;
import com.github.tartaricacid.touhoulittlemaid.entity.task.TaskManager;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.action.SearchBudget;
import com.lingchat.maidbridge.action.construction.BuildRules.CellState;
import com.lingchat.maidbridge.action.construction.BuildRules.Failure;
import com.lingchat.maidbridge.action.WanderGuard;
import com.lingchat.maidbridge.action.core.MaidActionController;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.debug.PathDebug;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.EventFrames;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
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
import net.minecraft.world.phys.Vec3;

import com.lingchat.maidbridge.action.construction.BuildMoves.Move;
import static com.lingchat.maidbridge.action.construction.BuildLayer.*;
import static com.lingchat.maidbridge.action.construction.BuildMoves.*;
import static com.lingchat.maidbridge.action.construction.BuildPlanner.*;

/**
 * 按图纸盖房子：走过每一格、够得着就放，没料或走不到就记下来，盖完推一条收尾事件。
 *
 * <p>与挖掘、放置一样是单槽位、每 tick 推进的异步活。施工顺序是先低层后高层，
 * 同一层里先放脚下有依托的格子；一遍放不完就再走一遍，让新放下的方块给邻居当依托。
 * 单格判据只看这一格此刻“是什么、够不够得着、放不放得下”，不经过 LLM。
 * 落子要求她眼睛到落点之间没有方块挡着；选站位时也只考虑看得见这一格的位置，
 * 墙背面只剩外侧地面可选。够得着却看不见的格子交给垫脚，一格一格把她送上去：
 * 优先往附近垫一格走上去，四面被堵或者目标就在正上方时原地跳起来把脚下补上。
 * 走不过去时先试跑跳：缺口另一头站得住、看得见这一格，就按平地四格、抬一格三格
 * 的规则跳过去，不必搭方块。够得着却站不过去时，就一格一格垫：目标高过两层时是往上爬，
 * 目标与她齐平或只高一格时，先把一格垫在脚下、走上去，站到放得下的地方。
 *
 * <p>一遍没放上的格子不急着判死：先把整张图纸过完，别的方块放好之后常常就能放了，
 * 到那时回头补试一轮，补过还是不行的才计入失败。
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
    /** 走路卡住这么久还没到落脚点，就认为这条路走不通。 */
    static final int STUCK_TICKS = 80;
    /** 这么久几乎没有位移才叫卡住：绕路时离目标变远是正常的，不能按直线距离判。 */
    static final int WALK_STILL_TICKS = 80;
    /** 从受理到收工最多等这么久（约四十分钟）：中型房子的块数多，二十分钟会半途收手。 */
    static final int BUILD_TIMEOUT_TICKS = 48000;
    /** 一 tick 最多判几格，避免第一次扫描时对整张图纸算寻路。 */
    static final int MAX_SCAN_PER_TICK = 6;
    /** 站到离目标这么近就能放（底面中心距离平方）。 */
    static final double AT_STAND = 4.0;
    /** 同一格最多换几个站位：换了这么多还看不见就认了，别在原地转圈。 */
    static final int MAX_STAND_TRIES = 4;
    /** 目标格被活物占着时先放一放，过这么久再试。 */
    static final int ENTITY_RETRY_TICKS = 100;
    /** 这一层全被活物占着时最多等这么久（约十秒）；再等下去不如先搁置换层。 */
    static final int HOLD_GIVEUP_TICKS = 200;
    /** 同一格连续因为寻路额度用完而没推进，最多等这么久（约六秒）。 */
    static final int MAX_CELL_WAITS = 120;
    /** 每一 tick 最多算几次寻路：屋里屋外不通时，一次全算完会把服务端卡住。 */
    static final int PATHS_PER_TICK = 6;
    /** 同一条跑跳路线没算通，隔这么久再试，别每 tick 都重算一遍。 */
    static final int HOP_RETRY_TICKS = 40;
    /** 同一格最多为她从高处跳下去两次：一次能落十格，落完还要往下也只再补一次。 */
    static final int MAX_DROPS = 2;
    /** 调试时多久算一次「下一块放哪」：每格都要验视线，不能每 tick 都算。 */
    static final int NEXT_PLACE_TICKS = 10;
    /** 原地起跳的初速：与玩家跳跃一致，刚好够她升过一整格。 */
    static final double JUMP_POWER = 0.42;
    /** 起跳后最多等这么久：没离地、也没把方块补进去就放手。 */
    static final int JUMP_TIMEOUT_TICKS = 20;
    /** 垫完一块之后等这么久再判下一格，让她先落回新方块上。 */
    static final int JUMP_LAND_TICKS = 10;
    /** 收尾时走去一个落脚点离开垫脚块，最多等这么久。 */
    static final int CLEANUP_TIMEOUT_TICKS = 120;
    /** 实在没有落脚点可去时，等这么久就把她脚下的垫脚块也拆掉，让她落回地面。 */
    static final int CLEANUP_WAIT_TICKS = 30;

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
        /** 为这一格试过、但没能放上的站位：换位置时跳过，避免来回走。 */
        final Set<BlockPos> triedStands = new HashSet<>();
        /** 为这一格垫过几次脚（按升高次数算），防止在原地反复搭。 */
        int steps;
        /** 为这一格平铺过几块路：齐平的缺口一块一块搭过去。 */
        int bridges;
        /** 为这一格从高处跳下去过几次：落地之后还要往下时，别在两级之间来回跳。 */
        int drops;
        /** 已经为这一格在目标底下垫过依托，再没有依托就真放不下了。 */
        boolean propped;
        /** 这一格被活物占着，等到这个游戏刻之后再试。 */
        long holdUntil;
        /** 这一格的跑跳路线没算通，等到这个游戏刻之后再算。 */
        long hopUntil;
        /** 连续因为寻路额度用完而停在这一格的 tick 数。 */
        int waits;
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
        /** 同一时刻只有一个动作：走路、垫脚、起跳垫高、跑跳。 */
        Move move;
        /** 她正赶过去或正踩着的落脚点；选格和收尾时用来护着那一格。 */
        BlockPos stand;
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
        /** 她为够到高处垫下的临时方块，她走开后逐块拆掉。 */
        final Map<BlockPos, Block> scaffold = new LinkedHashMap<>();
        /** 每块垫脚方块是为哪一格搭的：那一格还没放好之前不拆，拆了她就走不过去。 */
        final Map<BlockPos, Cell> scaffoldOwner = new HashMap<>();
        /** 已经做过一轮补试：剩下的格子再不行就真有定论了。 */
        boolean retried;
        /** 这一 tick 还剩几次寻路；每 tick 开头重新发一份。 */
        SearchBudget search = new SearchBudget(0);
        /** 这一 tick 已经算过一条跑跳路线：一条就够贵了。 */
        boolean hopPlanned;
        /** 从她上次站的这一格算过、走不到的站位：她没挪窝就不必反复算。 */
        final Set<BlockPos> unreachable = new HashSet<>();
        /** 上一次清站位缓存时她站的那一格。 */
        BlockPos searchFrom;
        /** 已经垫出的格数；它和 placed 一起算「这一遍有没有进展」。 */
        int stepPlaced;
        /** 正在处理的那一格：换了站位、退了再试时都先盯着它，别扫到别的格子去。 */
        Cell active;
        /** 为哪一格选了落脚点：她没到之前，脚下这一格先别放方块堵死。 */
        Cell standFor;
        /** 这一层的顺序该按她此刻的位置重排了：换层、重开一遍时各排一次。 */
        boolean needOrder;
        /** 施工完了先离开垫脚块、把它拆干净，再推收尾。 */
        boolean cleaning;
        int cleanTicks;
        BlockPos cleanStand;
        String pendingCode = "";
        String pendingReason = "";
        /** 走不开时拆得掉她脚下的那一块。 */
        boolean forceClean;
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
            return data;
        }
        data.addProperty("status", lastStatus.isEmpty() ? "unknown" : lastStatus);
        data.addProperty("name", lastName);
        data.addProperty("placed", lastPlaced);
        data.addProperty("total", lastTotal);
        data.addProperty("remaining", Math.max(0, lastTotal - lastPlaced));
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
                markDone(run, cell);
            } else if (state.getBlock() instanceof LiquidBlock) {
                settle(run, cell, Failure.LIQUID);
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
            run.stand = null;
            cancelMove(run, maid);
            run.forceClean = true;
            cleanScaffold(run, maid);
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
        if (run.cleaning) {
            if (PathDebug.watching(maid)) {
                reportDebug(run, maid);
            }
            stop("收尾");
            cleaning(server, run, maid);
            return;
        }
        if (++run.ticks > BUILD_TIMEOUT_TICKS) {
            finish(server, run, "timeout", "盖了这么久还没盖完，先停下了");
            return;
        }
        // 每个 tick 只给这么多寻路：屋里屋外不通时，逐格逐候选算路会把服务端拖住
        run.search = new SearchBudget(PATHS_PER_TICK);
        run.hopPlanned = false;
        if (run.nextPlaceIn > 0) {
            run.nextPlaceIn--;
        }
        // 她挪了地方，之前算不到的站位可能就能走到了：缓存按她脚下这一格作废
        if (!maid.blockPosition().equals(run.searchFrom)) {
            run.searchFrom = maid.blockPosition().immutable();
            run.unreachable.clear();
        }
        // 调试可视化开着时，把这一 tick 的阶段、落脚点与目标交上去；没开就只多一次空判断
        if (PathDebug.watching(maid)) {
            reportDebug(run, maid);
        }
        cleanScaffold(run, maid);
        if (run.cooldown > 0) {
            run.cooldown--;
            if (run.move != null && run.move.stand() != null) {
                MaidActuator.walkToPos(maid, run.move.stand(), 0);
            }
            stop("歇");
            return;
        }
        if (run.move != null) {
            advance(run, maid);
            stop("动");
            return;
        }
        if (run.needOrder) {
            run.needOrder = false;
            orderLayer(run, maid);
        }

        if (next(run, maid)) {
            return;
        }
        // 这一 tick 的寻路额度用完了、游标还停在这一层中间：下个 tick 接着扫，
        // 不能当成「这一遍没进展」——那会让好好的格子被提前收进失败清单
        if (run.cursor < run.layerEnd) {
            stop("扫");
            return;
        }
        endLayer(server, run);
    }




    /**
     * 调试可视化：这一 tick 她在干什么、正对着哪一格、站在哪、下一块大概放哪里。
     *
     * <p>白框标的是她手上这一格——走路时是目的地，放的时候就是手上这一块；空闲时才退回
     * 「下一块大概放哪」。以前这里取的是本层第一格没做的，和真正在放的那一格经常对不上。
     */
    static void reportDebug(Run run, EntityMaid maid) {
        predictNext(run, maid);
        Cell focus;
        String phase;
        if (run.cleaning) {
            phase = "收尾：收拾垫脚块";
            focus = null;
        } else if (run.move != null) {
            phase = run.move.phase();
            focus = run.move.cell();
        } else if (run.active != null && run.active.state.open()) {
            phase = "准备放这一格";
            focus = run.active;
        } else {
            phase = "扫描下一格";
            focus = null;
        }
        BlockPos target = focus != null ? focus.pos : run.nextPlace;
        Cell named = focus != null ? focus : cellAt(run, run.nextPlace);
        String block = named == null
                ? ""
                : BuiltInRegistries.BLOCK.getKey(named.block).toString();
        PathDebug.report(
                maid, phase, run.stand, target,
                run.move == null ? List.of() : run.move.chain(),
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
        int deferred = 0;
        for (Cell cell : run.cells) {
            if (cell.state == CellState.DEFERRED) {
                deferred++;
            }
        }
        long waited = run.heldSince == 0
                ? 0
                : Math.max(0, run.level.getGameTime() - run.heldSince) / 20;
        int pending = 0;
        int retrying = 0;
        int failed = 0;
        for (Cell cell : run.cells) {
            switch (cell.state) {
                case PENDING -> pending++;
                case RETRYING -> retrying++;
                case FAILED -> failed++;
                default -> {
                }
            }
        }
        String hand = run.active == null ? "-" : run.active.pos + ":" + run.active.state;
        return "｜施工 " + run.placed + "/" + run.total
                + "｜本层 开 " + open + "·等活物 " + waiting + "·预留 " + reserved
                + "·搁置 " + deferred
                + (waited > 0 ? "·已等 " + waited + "s" : "")
                + "｜全 待 " + pending + "·补 " + retrying + "·败 " + failed
                + "｜停 " + lastStop + "×" + stopTicks
                + "｜行 " + run.cursor + "/" + run.layerStart + "-" + run.layerEnd
                + "｜手 " + hand
                + "｜心跳 " + run.ticks
                + "｜额度 " + run.search.left() + "/" + run.search.total();
    }



    /** 收工阶段每 tick 推进：先走去落脚点，路上把垫脚块拆掉，拆干净再真正收尾。 */
    static void cleaning(MinecraftServer server, Run run, EntityMaid maid) {
        if (++run.cleanTicks > CLEANUP_TIMEOUT_TICKS) {
            run.forceClean = true;
            cleanScaffold(run, maid);
            finishNow(server, run, run.pendingCode, run.pendingReason);
            return;
        }
        if (run.cleanStand != null) {
            if (arrived(run.cleanStand, maid)) {
                run.cleanStand = null;
                MaidActuator.stopWalking(maid);
                // 已经站到实地上，剩下的垫脚块不用再护着，直接拆干净
                run.forceClean = true;
            } else {
                MaidActuator.walkToPos(maid, run.cleanStand, 0);
            }
        } else if (run.cleanTicks > CLEANUP_WAIT_TICKS) {
            // 没有落脚点可去：等一小会儿就把脚下的垫脚块也拆了，让她落回地面
            run.forceClean = true;
        }
        cleanScaffold(run, maid);
        if (run.scaffold.isEmpty()) {
            finishNow(server, run, run.pendingCode, run.pendingReason);
        }
    }

    /**
     * 收工时去哪站着：最近的一格能站人、又不踩着垫脚块的地面。
     *
     * <p>只在她身边几格内找；找不到给 null，由收尾阶段等一会儿直接拆。
     */
    static BlockPos awayStand(Run run, EntityMaid maid) {
        BlockPos here = maid.blockPosition();
        BlockPos best = null;
        double bestScore = Double.MAX_VALUE;
        for (int dx = -6; dx <= 6; dx++) {
            for (int dz = -6; dz <= 6; dz++) {
                for (int dy = -2; dy <= 2; dy++) {
                    BlockPos stand = here.offset(dx, dy, dz);
                    if (!cleanupStand(run, stand)) {
                        continue;
                    }
                    double score = stand.distSqr(here);
                    if (score < bestScore) {
                        bestScore = score;
                        best = stand;
                    }
                }
            }
        }
        return best;
    }

    static boolean cleanupStand(Run run, BlockPos stand) {
        if (run.scaffold.containsKey(stand) || run.scaffold.containsKey(stand.below())) {
            return false;
        }
        if (!run.level.isLoaded(stand) || !run.level.isLoaded(stand.above())) {
            return false;
        }
        if (!freeForStep(run, stand) || !freeForStep(run, stand.above())) {
            return false;
        }
        return solidCell(run.level, stand.below());
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
        if (natural && !run.cleaning && maid != null && !maid.isRemoved() && !run.scaffold.isEmpty()) {
            run.cleaning = true;
            run.cleanTicks = 0;
            run.pendingCode = reasonCode == null ? "" : reasonCode;
            run.pendingReason = reason == null ? "" : reason;
            run.stand = null;
            cancelMove(run, maid);
            run.cleanStand = awayStand(run, maid);
            if (run.cleanStand != null) {
                MaidActuator.walkToPos(maid, run.cleanStand, 0);
            }
            return;
        }
        finishNow(server, run, reasonCode, reason);
    }

    static void finishNow(MinecraftServer server, Run run, String reasonCode, String reason) {
        EntityMaid maid = MaidQueries.findMaidById(server, run.maidId);
        if (maid != null) {
            // 中断的路上不再护着脚下那块：垫出来的方块一块都不留
            run.stand = null;
            cancelMove(run, maid);
            run.forceClean = true;
            cleanScaffold(run, maid);
        }
        current = null;
        if (maid != null) {
            MaidActuator.stopWalking(maid);
            MaidActuator.stopLooking(maid);
            restoreTask(run, maid);
        }
        WanderGuard.end(run.maidId);

        // 补试阶段又落下的格子也要有个说法，别在失败清单里凭空消失
        for (Cell cell : run.cells) {
            if (cell.state == CellState.DEFERRED) {
                settle(run, cell, cell.failure);
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
                "place_failed", "blocked", "liquid", "out_of_reach")) {
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


    static boolean arrived(BlockPos stand, EntityMaid maid) {
        return arrived(stand, maid, false);
    }

    /**
     * 到没到落脚点。垫脚时要求她真的站进那一格，差半格不算——
     * 不然下一步会从斜角往对面跳，被旁边的方块挡住后再也上不去。
     */
    static boolean arrived(BlockPos stand, EntityMaid maid, boolean tight) {
        if (stand == null) {
            return false;
        }
        if (tight) {
            return maid.blockPosition().equals(stand);
        }
        return maid.position().distanceToSqr(Vec3.atBottomCenterOf(stand)) <= AT_STAND
                || maid.blockPosition().distManhattan(stand) <= 1;
    }

}
