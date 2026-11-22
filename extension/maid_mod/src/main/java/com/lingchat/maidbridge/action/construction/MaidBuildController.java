package com.lingchat.maidbridge.action.construction;

import com.github.tartaricacid.touhoulittlemaid.api.task.IMaidTask;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.github.tartaricacid.touhoulittlemaid.entity.task.TaskIdle;
import com.github.tartaricacid.touhoulittlemaid.entity.task.TaskManager;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.action.SearchBudget;
import com.lingchat.maidbridge.action.WanderGuard;
import com.lingchat.maidbridge.action.core.MaidActionController;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.action.mine.MaidMineController;
import com.lingchat.maidbridge.action.mine.MineRules;
import com.lingchat.maidbridge.action.mine.MineTargets;
import com.lingchat.maidbridge.action.place.Placement;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.maid.MaidInventory;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.EventFrames;
import com.lingchat.maidbridge.perception.item.ItemReporter;
import com.lingchat.maidbridge.perception.spatial.TerrainMemory;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.Predicate;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.BlockStateProperties;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;
import net.neoforged.neoforge.items.wrapper.CombinedInvWrapper;

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
    private static final org.slf4j.Logger LOGGER =
            org.slf4j.LoggerFactory.getLogger("lingchat_maid_bridge");
    /** 每放一块之间的基础间隔，再叠一点随机，避免像 for 循环一样一块接一块。 */
    private static final int PLACE_INTERVAL_BASE = 5;
    private static final int PLACE_INTERVAL_JITTER = 4;
    /** 下一格要转大角度时额外多等几 tick，让视线先转过去。 */
    private static final int TURN_EXTRA_TICKS = 5;
    /** 两遍之间歇一下。 */
    private static final int PASS_BREAK_TICKS = 2;
    /** 走路卡住这么久还没到落脚点，就认为这条路走不通。 */
    private static final int STUCK_TICKS = 80;
    /** 这么久几乎没有位移才叫卡住：绕路时离目标变远是正常的，不能按直线距离判。 */
    private static final int WALK_STILL_TICKS = 40;
    /** 从受理到收工最多等这么久（约二十分钟）。 */
    private static final int BUILD_TIMEOUT_TICKS = 24000;
    /** 一层连续几遍没有新进展就判这一层剩格不可达，换下一层。 */
    private static final int MAX_STALL_PASSES = 3;
    /** 一 tick 最多判几格，避免第一次扫描时对整张图纸算寻路。 */
    private static final int MAX_SCAN_PER_TICK = 6;
    /** 站到离目标这么近就能放（底面中心距离平方）。 */
    private static final double AT_STAND = 4.0;
    /** 同一格最多换几个站位：换了这么多还看不见就认了，别在原地转圈。 */
    private static final int MAX_STAND_TRIES = 4;
    /** 目标格被活物占着时先放一放，过这么久再试。 */
    private static final int ENTITY_RETRY_TICKS = 100;
    /** 每一 tick 最多算几次寻路：屋里屋外不通时，一次全算完会把服务端卡住。 */
    private static final int PATHS_PER_TICK = 6;
    /** 同一条跑跳路线没算通，隔这么久再试，别每 tick 都重算一遍。 */
    private static final int HOP_RETRY_TICKS = 40;
    /** 原地起跳的初速：与玩家跳跃一致，刚好够她升过一整格。 */
    private static final double JUMP_POWER = 0.42;
    /** 起跳后最多等这么久：没离地、也没把方块补进去就放手。 */
    private static final int JUMP_TIMEOUT_TICKS = 20;
    /** 垫完一块之后等这么久再判下一格，让她先落回新方块上。 */
    private static final int JUMP_LAND_TICKS = 10;
    /** 收尾时走去一个落脚点离开垫脚块，最多等这么久。 */
    private static final int CLEANUP_TIMEOUT_TICKS = 120;
    /** 实在没有落脚点可去时，等这么久就把她脚下的垫脚块也拆掉，让她落回地面。 */
    private static final int CLEANUP_WAIT_TICKS = 30;

    /** 图纸里的一格，已经换算到世界坐标。 */
    public record Target(BlockPos pos, Block block, Float yaw) {
    }

    private static final class Cell {
        final BlockPos pos;
        final Block block;
        final Item item;
        final String itemId;
        final Float yaw;
        boolean done;
        /** 这一轮不再为它努力：缺料、被挡、液体、走不到。 */
        boolean settled;
        /** 这一轮没放上，先记着：等别的格子都做完再回来补试。 */
        boolean deferred;
        /** 已经补试过一次：再不行就当真失败收尾，免得在同一个地方反复打转。 */
        boolean confirmed;
        /** 最后一次没放上的原因，等这一层整轮没进展时再收进失败清单。 */
        String lastFailure = "no_stand";
        /** 为这一格试过、但没能放上的站位：换位置时跳过，避免来回走。 */
        final Set<BlockPos> triedStands = new HashSet<>();
        /** 为这一格垫过几次脚（按升高次数算），防止在原地反复搭。 */
        int steps;
        /** 这一格被活物占着，等到这个游戏刻之后再试。 */
        long holdUntil;
        /** 这一格的跑跳路线没算通，等到这个游戏刻之后再算。 */
        long hopUntil;

        Cell(Target target) {
            this.pos = target.pos();
            this.block = target.block();
            this.item = target.block().asItem();
            this.itemId = BuiltInRegistries.ITEM.getKey(this.item).toString();
            this.yaw = target.yaw();
        }
    }

    private static final class Run {
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
        Cell walkingTo;
        BlockPos stand;
        /** 走向垫脚块必须真的踩上去：差半格就判到站会让她从斜角起跳。 */
        boolean tightArrival;
        int walkTicks;
        int walkStill;
        Vec3 walkLast;
        /** 当前处理的层在格子表里的下标区间。 */
        int layerStart;
        int layerEnd;
        int cursor;
        int layerPassPlaced;
        int stallPasses;
        /** 图纸里已经放好的格子，用来优先站到自己盖出来的面上。 */
        final Set<BlockPos> built = new HashSet<>();
        /** 图纸占着的格子：垫脚不能占掉图纸自己的位置。 */
        final Set<BlockPos> blueprint = new HashSet<>();
        /** 她为够到高处垫下的临时方块，她走开后逐块拆掉。 */
        final Map<BlockPos, Block> scaffold = new LinkedHashMap<>();
        /** 垫脚还差顶上那一块：这一 tick 先放托底，下一 tick 再放它并走上去。 */
        BlockPos stepTop;
        Cell stepFor;
        Item stepItem;
        /** 正在原地起跳往上垫：跳起来之后把方块补在她原来站的那一格。 */
        Cell jumpFor;
        BlockPos jumpPos;
        Item jumpItem;
        int jumpTicks;
        /** 正在跑的一段跳：跨过缺口时用它替代垫脚。 */
        MaidParkour.Runner hop;
        Cell hopCell;
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

        Run(EntityMaid maid, String name, List<Cell> cells, boolean consumeMaterials) {
            this.maidId = maid.getStringUUID();
            this.level = (ServerLevel) maid.level();
            this.name = name;
            this.cells = cells;
            this.total = cells.size();
            this.consumeMaterials = consumeMaterials;
        }
    }

    private static Run current;
    /** 上一张图纸的锚点与状态：补料后重调时用它接着原来的工地，而不是按新朝向另起一座。 */
    private static String lastSignature = "";
    private static BlockPos lastAnchor;
    private static int lastBaseY;
    private static ServerLevel lastLevel;
    private static String lastDimension = "";
    private static String lastStatus = "";
    private static String lastName = "";
    private static int lastPlaced;
    private static int lastTotal;
    /** 上一张图纸已经换算好的世界坐标；接着盖时原样复用，免得旋转后的图纸和半成品对不上。 */
    private static List<Target> lastTargets = List.of();

    private MaidBuildController() {
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
            if (cell.block == state.getBlock()) {
                cell.done = true;
                run.built.add(cell.pos);
                run.placed++;
            } else if (state.getBlock() instanceof LiquidBlock) {
                settle(run, cell, "liquid");
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
            clearJump(run);
            clearHop(run, maid);
            run.forceClean = true;
            cleanScaffold(run, maid);
            MaidActuator.stopWalking(maid);
            MaidActuator.stopLooking(maid);
            restoreTask(run, maid);
        }
        WanderGuard.end(run.maidId);
    }

    /** 把她施工前的工作模式换回去。 */
    private static void restoreTask(Run run, EntityMaid maid) {
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
        // 她挪了地方，之前算不到的站位可能就能走到了：缓存按她脚下这一格作废
        if (!maid.blockPosition().equals(run.searchFrom)) {
            run.searchFrom = maid.blockPosition().immutable();
            run.unreachable.clear();
        }
        cleanScaffold(run, maid);
        if (run.cooldown > 0) {
            run.cooldown--;
            if (run.walkingTo != null) {
                MaidActuator.walkToPos(maid, run.stand, 0);
            }
            return;
        }
        if (run.stepTop != null) {
            topStep(run, maid);
            return;
        }
        if (run.jumpFor != null) {
            jumping(run, maid);
            return;
        }
        if (run.hop != null) {
            hopping(run, maid);
            return;
        }
        if (run.walkingTo != null) {
            walking(run, maid);
            return;
        }

        if (next(run, maid)) {
            return;
        }
        // 这一 tick 的寻路额度用完了、游标还停在这一层中间：下个 tick 接着扫，
        // 不能当成「这一遍没进展」——那会让好好的格子被提前收进失败清单
        if (run.cursor < run.layerEnd) {
            return;
        }
        endLayer(server, run);
    }

    /** 走到落脚点就试着放；走不到就记这一格，不把整栋楼耗在一条路上。 */
    private static void walking(Run run, EntityMaid maid) {
        if (arrived(run.stand, maid, run.tightArrival)) {
            Cell cell = run.walkingTo;
            run.walkingTo = null;
            tryPlace(run, maid, cell);
            return;
        }
        // 绕路时离目标变远是正常的，只看她自己有没有真的在动
        Vec3 now = maid.position();
        if (run.walkLast != null && now.distanceToSqr(run.walkLast) < 0.0025) {
            run.walkStill++;
        } else {
            run.walkStill = 0;
        }
        run.walkLast = now;
        boolean noPath = ++run.walkTicks > STUCK_TICKS
                && maid.getBrain().hasMemoryValue(MemoryModuleType.CANT_REACH_WALK_TARGET_SINCE);
        if (noPath || run.walkStill > WALK_STILL_TICKS) {
            Cell cell = run.walkingTo;
            run.walkingTo = null;
            if (cell != null && run.stand != null) {
                cell.triedStands.add(run.stand);
            }
            // 走过去的路没了：缺口另一头若在跑跳范围内，跳过去比垫方块省
            if (cell != null && hopToPlace(run, maid, cell)) {
                return;
            }
            // 被屋顶或楼板压在里面时，寻路本来就上不去也出不来：先走到露天的地方
            if (cell != null && stageOutside(run, maid, cell)) {
                return;
            }
            if (run.search.spent()) {
                // 这一 tick 的寻路额度用完了，下一 tick 再判，别急着记账
                run.cooldown = PASS_BREAK_TICKS;
                return;
            }
            temporary(run, cell, canJumpHere(run, maid) ? "no_path" : "no_headroom");
            run.cooldown = PASS_BREAK_TICKS;
            return;
        }
        MaidActuator.walkToPos(maid, run.stand, 0);
    }

    /** 换一个站位：重置走路进度，下一次 tick 开始走。 */
    private static void startWalk(Run run, EntityMaid maid, Cell cell, BlockPos stand, boolean tight) {
        run.stand = stand;
        run.walkingTo = cell;
        run.tightArrival = tight;
        run.walkTicks = 0;
        run.walkStill = 0;
        run.walkLast = maid.position();
        MaidActuator.walkToPos(maid, stand, 0);
    }

    /** 这一格被活物占着，还没到重试的时候。 */
    private static boolean held(Cell cell, ServerLevel level) {
        return cell.holdUntil > level.getGameTime();
    }

    /** 在屋里被天花板顶住时，先去附近一处露天的地方，再沿外墙往上垫。 */
    private static boolean stageOutside(Run run, EntityMaid maid, Cell cell) {
        if (run.level.canSeeSky(maid.blockPosition().above())) {
            return false;
        }
        BlockPos stage = MineTargets.stageFor(maid, cell.pos, run.search);
        if (stage == null) {
            return false;
        }
        startWalk(run, maid, cell, stage, false);
        return true;
    }

    /** 她此刻能不能起跳：脚与头两格之外，头顶上面还要再让出一格。 */
    private static boolean canJumpHere(Run run, EntityMaid maid) {
        BlockPos feet = maid.blockPosition();
        return freeForStep(run, feet.above()) && freeForStep(run, feet.above(2));
    }

    /** 这一 tick 的寻路额度花完了：这一格留到下个 tick，别按失败记账。 */
    private static boolean budgetOut(Run run) {
        if (!run.search.spent()) {
            return false;
        }
        if (run.cursor > run.layerStart) {
            run.cursor--;
        }
        return true;
    }

    /** 从当前游标往后找一格现在能处理的；处理了（放了或开始走过去）返回真。 */
    private static boolean next(Run run, EntityMaid maid) {
        // 先把站在原地够得着的格子放掉，别为了一格远处的跑过去又跑回来
        if (placeReachable(run, maid)) {
            return true;
        }
        int scanned = 0;
        boolean doorsOnly = onlyDoorsLeft(run);
        while (run.cursor < run.layerEnd && scanned < MAX_SCAN_PER_TICK) {
            // 寻路额度用完就停在这里：剩下的格子下个 tick 再扫，不能因为算不动就记失败
            if (run.search.spent()) {
                break;
            }
            scanned++;
            Cell cell = run.cells.get(run.cursor++);
            if (cell.done || cell.settled || held(cell, run.level)) {
                continue;
            }
            // 门留到别的格子都放好之后再装，施工期间那个门洞一直敞着，她好进出
            if (!doorsOnly && cell.block == Blocks.OAK_DOOR) {
                continue;
            }
            BlockState state = run.level.getBlockState(cell.pos);
            if (cell.block == state.getBlock()) {
                markDone(run, cell);
                continue;
            }
            if (state.getBlock() instanceof LiquidBlock) {
                settle(run, cell, "liquid");
                continue;
            }
            if (!state.isAir() && !state.canBeReplaced()) {
                // 被别的方块占着：够得着就挖掉（掉落进她背包），够不着先走过去
                if (!maid.canDestroyBlock(cell.pos)) {
                    defer(run, cell, "blocked");
                    continue;
                }
                if (MineTargets.canMine(maid, cell.pos)) {
                    clearCell(run, maid, cell);
                    return true;
                }
                BlockPos dig = MineTargets.standFor(maid, cell.pos);
                if (dig == null) {
                    temporary(run, cell, "no_stand");
                    continue;
                }
                startWalk(run, maid, cell, dig, false);
                return true;
            }
            if (run.consumeMaterials && MaidInventory.count(maid, cell.item) <= 0) {
                missing(run, cell);
                continue;
            }
            if (otherEntityInside(run, cell.pos, maid)) {
                // 别的活物挡着：先不管，过一会儿自动重试
                cell.holdUntil = run.level.getGameTime() + ENTITY_RETRY_TICKS;
                temporary(run, cell, "blocked_by_entity");
                continue;
            }
            if (selfInside(cell.pos, maid)) {
                // 自己站在要放的那一格：先挪出去，别把自己埋了
                BlockPos away = MineTargets.standForBuilding(
                        maid, cell.pos, run.built, cell.triedStands, run.unreachable,
                        stand -> true, run.search);
                if (away != null && !away.equals(maid.blockPosition())) {
                    startWalk(run, maid, cell, away, false);
                    return true;
                }
                if (budgetOut(run)) {
                    return true;
                }
                temporary(run, cell, "self_blocked");
                continue;
            }
            // 依托面还没出现（屋顶中间那格要等旁边先放）：先跳过，不算这一格失败，
            // 否则试满六次就被当成够不着，后面放好的邻居也救不回来
            BlockPlaceContext context = context(maid, cell);
            if (context == null) {
                continue;
            }
            if (withinReach(maid, cell.pos)) {
                // 够得着还要看得见：隔着一面墙、一根柱子伸过去是不行的
                BlockPlaceContext visible = visibleContext(maid, cell);
                if (visible != null) {
                    place(run, maid, cell, visible);
                    return true;
                }
                // 够得着却看不见：换一个看得见这一格的站位再试
                Predicate<BlockPos> canSee = stand -> Placement.seesFrom(maid, stand, cell.pos, null);
                BlockPos other = MineTargets.standForBuilding(
                        maid, cell.pos, run.built, cell.triedStands, run.unreachable, canSee,
                        run.search);
                if (other != null && !other.equals(maid.blockPosition())
                        && cell.triedStands.size() < MAX_STAND_TRIES) {
                    startWalk(run, maid, cell, other, false);
                    return true;
                }
                if (budgetOut(run)) {
                    return true;
                }
                // 换不到站位：缺口另一头够得着、又站得住的话，先试试跑跳过去
                if (hopToPlace(run, maid, cell)) {
                    return true;
                }
                // 还是不行就再垫一格把眼睛抬高：柱子顶上那几格只能这么看见
                if (stepUp(run, maid, cell)) {
                    return true;
                }
                if (stageOutside(run, maid, cell)) {
                    return true;
                }
                temporary(run, cell, canJumpHere(run, maid) ? "no_sight" : "no_headroom");
                continue;
            }
            Predicate<BlockPos> canSee = stand -> Placement.seesFrom(maid, stand, cell.pos, null);
            BlockPos stand = MineTargets.standForBuilding(
                    maid, cell.pos, run.built, cell.triedStands, run.unreachable, canSee,
                    run.search);
            if (stand == null) {
                if (budgetOut(run)) {
                    return true;
                }
                // 没有能走过去的站位：缺口另一头若一跳能到，就不用垫方块
                if (hopToPlace(run, maid, cell)) {
                    return true;
                }
                if (stepUp(run, maid, cell)) {
                    return true;
                }
                if (stageOutside(run, maid, cell)) {
                    return true;
                }
                temporary(run, cell, canJumpHere(run, maid) ? "no_stand" : "no_headroom");
                continue;
            }
            startWalk(run, maid, cell, stand, false);
            return true;
        }
        return false;
    }

    /** 站在原地伸手够得着的格子先放掉；一格都放不了返回假。 */
    private static boolean placeReachable(Run run, EntityMaid maid) {
        boolean doorsOnly = onlyDoorsLeft(run);
        for (int i = run.layerStart; i < run.layerEnd; i++) {
            Cell cell = run.cells.get(i);
            if (cell.done || cell.settled || held(cell, run.level)) {
                continue;
            }
            if (!doorsOnly && cell.block == Blocks.OAK_DOOR) {
                continue;
            }
            BlockState state = run.level.getBlockState(cell.pos);
            if (cell.block == state.getBlock()) {
                markDone(run, cell);
                continue;
            }
            if (state.getBlock() instanceof LiquidBlock) {
                settle(run, cell, "liquid");
                continue;
            }
            if (!state.isAir() && !state.canBeReplaced()) {
                // 留给 next()：够得着就挖掉，够不着先走过去
                continue;
            }
            if (run.consumeMaterials && MaidInventory.count(maid, cell.item) <= 0) {
                missing(run, cell);
                continue;
            }
            if (!withinReach(maid, cell.pos) || selfInside(cell.pos, maid)
                    || otherEntityInside(run, cell.pos, maid)) {
                continue;
            }
            BlockPlaceContext visible = visibleContext(maid, cell);
            if (visible != null) {
                place(run, maid, cell, visible);
                return true;
            }
        }
        return false;
    }

    private static void tryPlace(Run run, EntityMaid maid, Cell cell) {
        BlockState state = run.level.getBlockState(cell.pos);
        if (cell.block == state.getBlock()) {
            markDone(run, cell);
            return;
        }
        if (state.getBlock() instanceof LiquidBlock) {
            settle(run, cell, "liquid");
            return;
        }
        if (!state.isAir() && !state.canBeReplaced()) {
            // 走过来发现这一格被别的方块占着：挖掉它，下一遍再放
            if (maid.canDestroyBlock(cell.pos) && MineTargets.canMine(maid, cell.pos)) {
                clearCell(run, maid, cell);
            } else {
                temporary(run, cell, "blocked");
            }
            return;
        }
        if (run.consumeMaterials && MaidInventory.count(maid, cell.item) <= 0) {
            missing(run, cell);
            return;
        }
        if (otherEntityInside(run, cell.pos, maid)) {
            // 别的活物挡着：先放一放，过一会儿自动重试
            cell.holdUntil = run.level.getGameTime() + ENTITY_RETRY_TICKS;
            temporary(run, cell, "blocked_by_entity");
            return;
        }
        if (selfInside(cell.pos, maid)) {
            // 还站在这一格里：等下一步走开再放，不记失败
            return;
        }
        if (!withinReach(maid, cell.pos)) {
            // 垫上去之后可能还差一点：别在超出玩家距离的地方把方块放出来
            temporary(run, cell, "out_of_reach");
            return;
        }
        BlockPlaceContext visible = visibleContext(maid, cell);
        if (visible == null) {
            // 依托面还没出现，或者目标被别的方块挡住：下一遍再试，不记失败次数
            if (context(maid, cell) == null) {
                return;
            }
            if (run.stand != null) {
                cell.triedStands.add(run.stand);
            }
            temporary(run, cell, "no_sight");
            return;
        }
        place(run, maid, cell, visible);
    }

    private static void place(Run run, EntityMaid maid, Cell cell, BlockPlaceContext context) {
        MaidActuator.stopWalking(maid);
        MaidActuator.lookAtPos(maid, cell.pos);
        ((BlockItem) cell.item).place(context);
        BlockState after = run.level.getBlockState(cell.pos);
        if (cell.block != after.getBlock()) {
            temporary(run, cell, "place_failed");
            return;
        }
        upright(run.level, cell.pos);
        if (run.consumeMaterials && MaidInventory.takeOne(maid, cell.item)) {
            // 少掉的是她放下去的那一块，账上记一笔：不然背包差分把这次消耗说成主人拿走了
            ItemReporter.markRemoval(cell.itemId, 1);
        }
        maid.swing(InteractionHand.MAIN_HAND);
        markDone(run, cell);
        run.cooldown = placeInterval(run, maid, cell);
        // 世界变了，之前算不到的站位可能就能走到了
        run.unreachable.clear();
    }

    /**
     * 把目标格里的别的方块挖掉：工具、耐久、掉落都按挖掘那一套来。
     *
     * <p>车万女仆的 {@code destroyBlock} 会把掉落直接塞进她的库存；这一笔是她自己挖的，
     * 让背包差分别再报成「主人放进来的」。
     */
    private static void clearCell(Run run, EntityMaid maid, Cell cell) {
        BlockState state = run.level.getBlockState(cell.pos);
        if (!maid.canDestroyBlock(cell.pos)) {
            defer(run, cell, "blocked");
            return;
        }
        MineTargets.equipBestTool(maid, state);
        MaidActuator.stopWalking(maid);
        MaidActuator.lookAtPos(maid, cell.pos);
        if (!maid.destroyBlock(cell.pos, true)) {
            defer(run, cell, "blocked");
            return;
        }
        maid.swing(InteractionHand.MAIN_HAND);
        ItemStack used = maid.getMainHandItem();
        if (!used.isEmpty()) {
            used.hurtAndBreak(1, maid, EquipmentSlot.MAINHAND);
        }
        TerrainMemory.reveal(maid, cell.pos, run.level.getGameTime());
        MaidMineController.markMined(maid);
        // 世界变了，之前算不到的站位可能就能走到了
        run.unreachable.clear();
    }

    /** 原木这类有朝向轴的方块强制竖着放，不按她点的那一面横过来。 */
    private static void upright(ServerLevel level, BlockPos pos) {
        BlockState state = level.getBlockState(pos);
        if (!state.hasProperty(BlockStateProperties.AXIS)
                || state.getValue(BlockStateProperties.AXIS) == Direction.Axis.Y) {
            return;
        }
        level.setBlock(pos, state.setValue(BlockStateProperties.AXIS, Direction.Axis.Y), 3);
    }

    /**
     * 每块之间留一点随机间隔；下一格要转的角度越大，多等几 tick 让视线先转过去。
     *
     * <p>固定间隔会让整栋楼像机器刷出来的一样；视线从墙这头跳到那头时，她的头还没
     * 转过去，下一块就已经放上了。
     */
    private static int placeInterval(Run run, EntityMaid maid, Cell placed) {
        int interval = PLACE_INTERVAL_BASE + run.level.random.nextInt(PLACE_INTERVAL_JITTER);
        Cell next = nextOpenCell(run);
        if (next == null || next == placed) {
            return interval;
        }
        // 先把视线挪到下一格，冷却这段时间她正好转头；不然头还没动方块就出来了
        MaidActuator.lookAtPos(maid, next.pos);
        Vec3 eye = maid.getEyePosition();
        Vec3 from = Vec3.atCenterOf(placed.pos).subtract(eye);
        Vec3 to = Vec3.atCenterOf(next.pos).subtract(eye);
        if (from.lengthSqr() < 1.0E-4 || to.lengthSqr() < 1.0E-4) {
            return interval;
        }
        double cosine = from.normalize().dot(to.normalize());
        double angle = Math.acos(Math.max(-1.0, Math.min(1.0, cosine)));
        int extra = (int) Math.round(angle / (Math.PI / 6));
        return interval + Math.min(TURN_EXTRA_TICKS, extra);
    }

    /** 接下来最可能放的那一格，只用来估视线要转多少；本层放完了就往后找。 */
    private static Cell nextOpenCell(Run run) {
        boolean doorsOnly = onlyDoorsLeft(run);
        for (int i = run.layerStart; i < run.layerEnd; i++) {
            Cell cell = run.cells.get(i);
            if (cell.done || cell.settled) {
                continue;
            }
            if (!doorsOnly && cell.block == Blocks.OAK_DOOR) {
                continue;
            }
            return cell;
        }
        for (int i = run.layerEnd; i < run.cells.size(); i++) {
            Cell cell = run.cells.get(i);
            if (cell.done || cell.settled) {
                continue;
            }
            if (!doorsOnly && cell.block == Blocks.OAK_DOOR) {
                continue;
            }
            return cell;
        }
        if (doorsOnly) {
            for (Cell cell : run.cells) {
                if (!cell.done && !cell.settled && cell.block == Blocks.OAK_DOOR) {
                    return cell;
                }
            }
        }
        return null;
    }

    /**
     * 够不着时在她脚下垫一格：升上去、或者平着挪到能放的位置。
     *
     * <p>她的寻路只能上一格（再多就要跳跃输入），所以每 tick 只垫一格：先在她脚下
     * 这一层找一格空位，把方块放进去，再把寻路目标设到新方块的顶上。下一 tick 她
     * 站高一格，再判还差多少。目标还高过两层时，每垫一格都必须比现在更接近目标；
     * 目标与她齐平或只高一格时（屋顶那一步大多是这种），只要垫完放得下就垫，
     * 方向可以偏离目标——边上往往只有那一侧是空的。
     *
     * <p>托底那块先垫：顶块立在一块空地上放不住，得有下面的方块给它当依托。
     * 四面没有能垫的位置时改原地起跳，见 {@link #jumpUp}。
     */
    private static boolean stepUp(Run run, EntityMaid maid, Cell cell) {
        if (cell.steps >= ScaffoldRules.MAX_STEPS
                || run.scaffold.size() >= ScaffoldRules.MAX_BLOCKS) {
            return false;
        }
        BlockPos feet = maid.blockPosition();
        int rise = cell.pos.getY() - feet.getY();
        if (!ScaffoldRules.mayStep(rise)) {
            return false;
        }
        // 头顶被方块压住时她既跳不起来也迈不上去：先走开（交给 stageOutside），别在原地耗
        if (!canJumpHere(run, maid)) {
            return false;
        }
        if (!ScaffoldRules.nearEnough(
                cell.pos.getX() - feet.getX(), cell.pos.getZ() - feet.getZ())) {
            return false;
        }
        Item item = scaffoldItem(run, maid);
        if (item == null) {
            return false;
        }
        Vec3 target = Vec3.atCenterOf(cell.pos);
        double current = Vec3.atBottomCenterOf(feet).distanceToSqr(target);
        // 目标就在正上方：原地跳起来把脚下垫高，别为了垫一格先横着挪出去再爬回来
        if (rise >= 2 && cell.pos.getX() == feet.getX() && cell.pos.getZ() == feet.getZ()
                && jumpUp(run, maid, cell)) {
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
            if (!stepRoom(run, maid, top)) {
                continue;
            }
            double score = Vec3.atBottomCenterOf(top.above()).distanceToSqr(target);
            if (ScaffoldRules.mustApproach(rise) && score > current - 0.5) {
                continue;
            }
            BlockPos support = top.below();
            boolean supportReady = solidCell(run.level, support);
            if (!supportReady) {
                if (run.blueprint.contains(support) || !freeForStep(run, support)
                        || Placement.context(
                                        maid, new ItemStack(item), support, null, null, null)
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
            if (canPlaceFrom(maid, top.above(), cell.pos) && cost < placeCost) {
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
        // 垫完就能放的站位优先：目标与她齐平时，这一格就是为了站过去
        if (placeTop != null) {
            bestTop = placeTop;
            bestSupportReady = placeSupportReady;
        }
        if (bestTop == null) {
            // 四个方向都垫不出去（贴着墙、被围住）：只能原地往上垫
            return jumpUp(run, maid, cell);
        }
        if (!bestSupportReady) {
            if (!put(run, maid, bestTop.below(), item)) {
                return false;
            }
            // 托底只是前半个动作，升高数等到顶块放上去才算
            run.stepTop = bestTop;
            run.stepFor = cell;
            run.stepItem = item;
            run.cooldown = PASS_BREAK_TICKS;
            return true;
        }
        if (!put(run, maid, bestTop, item)) {
            return false;
        }
        cell.steps++;
        climbOnto(run, maid, cell, bestTop.above());
        return true;
    }

    /** 上一 tick 已经垫好托底，这一 tick 把顶块放上再走上去。 */
    private static void topStep(Run run, EntityMaid maid) {
        BlockPos top = run.stepTop;
        Cell cell = run.stepFor;
        Item item = run.stepItem;
        run.stepTop = null;
        run.stepFor = null;
        run.stepItem = null;
        if (top == null || cell == null || item == null) {
            return;
        }
        if (cell.steps >= ScaffoldRules.MAX_STEPS) {
            return;
        }
        if (!stepRoom(run, maid, top) || !put(run, maid, top, item)) {
            return;
        }
        cell.steps++;
        climbOnto(run, maid, cell, top.above());
    }

    /**
     * 原地跳起来把脚下的方块补上：站在一格空位里起跳，脚一离地就把方块放进原来那一格，
     * 落下来就升了一整格。
     *
     * <p>四个方向都垫不出去（贴着墙、被围住），或者目标就在正上方时只能这么往上，玩家
     * 搭柱子也是这个手法。起跳要有三格净空：脚一格、头一格、跳起来还要一格，否则头先撞上
     * 天花板，一格也升不上去。
     */
    private static boolean jumpUp(Run run, EntityMaid maid, Cell cell) {
        if (cell.steps >= ScaffoldRules.MAX_STEPS
                || run.scaffold.size() >= ScaffoldRules.MAX_BLOCKS) {
            return false;
        }
        if (!maid.onGround()) {
            return false;
        }
        BlockPos feet = maid.blockPosition();
        if (run.blueprint.contains(feet)
                || !run.level.getBlockState(feet).canBeReplaced()
                || !freeForStep(run, feet.above())
                || !freeForStep(run, feet.above(2))
                || !solidCell(run.level, feet.below())
                || otherEntityInside(run, feet, maid)) {
            return false;
        }
        Item item = scaffoldItem(run, maid);
        if (item == null) {
            return false;
        }
        MaidActuator.stopWalking(maid);
        MaidActuator.lookAtPos(maid, cell.pos);
        maid.setDeltaMovement(0.0, JUMP_POWER, 0.0);
        // 人一离地，脚下这块就不在碰撞盒里了：先把她要落脚的那一格记成站位，
        // 收尾拆垫脚块时才会护着它，不会刚放上去就拆掉
        run.stand = feet.above().immutable();
        run.jumpFor = cell;
        run.jumpPos = feet.immutable();
        run.jumpItem = item;
        run.jumpTicks = 0;
        return true;
    }

    /** 起跳之后盯着她离地：脚升过一整格，就把方块补进原来站的那一格。 */
    private static void jumping(Run run, EntityMaid maid) {
        Cell cell = run.jumpFor;
        BlockPos pos = run.jumpPos;
        if (cell == null || pos == null) {
            clearJump(run);
            return;
        }
        if (++run.jumpTicks > JUMP_TIMEOUT_TICKS) {
            // 跳不起来（头顶被封、被别的模组按住）：这一格换别的办法
            clearJump(run);
            temporary(run, cell, "no_stand");
            return;
        }
        if (maid.onGround()) {
            if (!maid.blockPosition().equals(pos) || run.jumpTicks > 3) {
                // 被挤开了，或者推了几下都没离地
                clearJump(run);
                temporary(run, cell, "no_stand");
                return;
            }
            maid.setDeltaMovement(0.0, JUMP_POWER, 0.0);
            return;
        }
        if (maid.getY() < pos.getY() + 1.02) {
            return;
        }
        // 人已经离开那一格：把方块补进去，落下来就踩在这一块上
        Item item = run.jumpItem;
        clearJump(run);
        if (!put(run, maid, pos, item)) {
            temporary(run, cell, "place_failed");
            return;
        }
        cell.steps++;
        run.cooldown = JUMP_LAND_TICKS;
    }

    private static void clearJump(Run run) {
        run.jumpFor = null;
        run.jumpPos = null;
        run.jumpItem = null;
        run.jumpTicks = 0;
    }

    /** 跑跳的时候按弧线摆位置，走完这一段再决定放不放。 */
    private static void hopping(Run run, EntityMaid maid) {
        MaidParkour.Runner hop = run.hop;
        hop.tick();
        if (hop.failed()) {
            Cell cell = run.hopCell;
            run.hop = null;
            run.hopCell = null;
            if (cell != null) {
                // 这条路线走不通：记下来，换别的落点或者退回垫脚
                cell.triedStands.addAll(hop.chain());
                temporary(run, cell, "no_path");
            }
            run.cooldown = PASS_BREAK_TICKS;
            return;
        }
        if (!hop.done()) {
            return;
        }
        Cell cell = run.hopCell;
        run.hop = null;
        run.hopCell = null;
        run.cooldown = PASS_BREAK_TICKS;
        LOGGER.info("施工跑跳：{} 跳后落在 {}", hop.chain().size(), maid.blockPosition());
        // 站定之后就是放这一格；失败时 triedStands 会记住这个落点，下一轮换别处
        run.stand = maid.blockPosition();
        if (cell != null) {
            tryPlace(run, maid, cell);
        }
    }

    /**
     * 从她脚下找一条跑跳路线，落到能放下这一格的站位上。
     *
     * <p>只在走不过去之后试：先算落点够不够得着、看不看得见，再验一次跳过不过得去。
     * 路线上的每一跳都按「平地四格、抬一格三格、往下最多三格」算。
     */
    private static boolean hopToPlace(Run run, EntityMaid maid, Cell cell) {
        if (run.hop != null || !maid.onGround() || run.hopPlanned) {
            return false;
        }
        // 一条路线要逐格验视线，很贵：同一 tick 只算一条，同一步没算通隔两秒再试
        if (cell.hopUntil > run.level.getGameTime()) {
            return false;
        }
        run.hopPlanned = true;
        List<BlockPos> chain = MaidParkour.planToPlace(maid, cell.pos, cell.triedStands,
                stand -> canPlaceFrom(maid, stand, cell.pos));
        if (chain.isEmpty()) {
            cell.hopUntil = run.level.getGameTime() + HOP_RETRY_TICKS;
            return false;
        }
        run.walkingTo = null;
        run.stand = null;
        run.hopCell = cell;
        run.hop = new MaidParkour.Runner(maid, chain);
        if (run.hop.failed()) {
            cell.triedStands.addAll(chain);
            cell.hopUntil = run.level.getGameTime() + HOP_RETRY_TICKS;
            run.hop = null;
            run.hopCell = null;
            return false;
        }
        return true;
    }

    private static void clearHop(Run run, EntityMaid maid) {
        if (run.hop != null) {
            run.hop.cancel();
            run.hop = null;
        }
        run.hopCell = null;
    }

    /** 站到这一格之后够得着、也看得见目标吗：垫这一脚、跳这一步到底有没有用。 */
    private static boolean canPlaceFrom(EntityMaid maid, BlockPos stand, BlockPos target) {
        if (stand.equals(target)) {
            return false;
        }
        Vec3 eye = Vec3.atBottomCenterOf(stand).add(0.0, maid.getEyeHeight(), 0.0);
        if (new AABB(target).distanceToSqr(eye) > MineRules.REACH * MineRules.REACH) {
            return false;
        }
        return Placement.seesFrom(maid, stand, target, null);
    }

    /** 这一格能不能当垫脚石：位置空着、头顶有地方站、不在图纸里、她伸手够得着。 */
    private static boolean stepRoom(Run run, EntityMaid maid, BlockPos top) {
        if (run.blueprint.contains(top) || !freeForStep(run, top)) {
            return false;
        }
        // 站上去要两格高：脚一格、头一格，否则会被房顶卡住
        if (!freeForStep(run, top.above()) || !freeForStep(run, top.above(2))) {
            return false;
        }
        return !selfInside(top, maid)
                && !otherEntityInside(run, top, maid)
                && !otherEntityInside(run, top.above(), maid)
                && withinReach(maid, top);
    }

    private static boolean freeForStep(Run run, BlockPos pos) {
        BlockState state = run.level.getBlockState(pos);
        if (!state.isAir() && !state.canBeReplaced()) {
            return false;
        }
        return !(state.getBlock() instanceof LiquidBlock);
    }

    private static void climbOnto(Run run, EntityMaid maid, Cell cell, BlockPos stand) {
        // 必须真的踩到新方块上：差一格就判到站，她会从斜角起跳，落在原来高度的方块上
        startWalk(run, maid, cell, stand, true);
    }

    /** 垫脚用的方块：优先她自己背着的整方块；创造模式她背包空着时借一块圆石。 */
    private static Item scaffoldItem(Run run, EntityMaid maid) {
        Item carried = MaidInventory.scaffoldBlock(maid);
        if (carried != null) {
            return carried;
        }
        return run.consumeMaterials ? null : Blocks.COBBLESTONE.asItem();
    }

    /** 放一块垫脚方块：扣料、记账，她走开后由 cleanScaffold 拆掉。 */
    private static boolean put(Run run, EntityMaid maid, BlockPos pos, Item item) {
        if (run.scaffold.size() >= ScaffoldRules.MAX_BLOCKS) {
            return false;
        }
        if (run.consumeMaterials && !MaidInventory.has(maid, item)) {
            return false;
        }
        if (!freeForStep(run, pos) || selfInside(pos, maid) || otherEntityInside(run, pos, maid)) {
            return false;
        }
        Block block = ((BlockItem) item).getBlock();
        BlockPlaceContext context =
                Placement.visibleContext(maid, new ItemStack(item), pos, null, null, null);
        if (context == null) {
            return false;
        }
        MaidActuator.stopWalking(maid);
        MaidActuator.lookAtPos(maid, pos);
        ((BlockItem) item).place(context);
        if (run.level.getBlockState(pos).getBlock() != block) {
            return false;
        }
        upright(run.level, pos);
        if (run.consumeMaterials && MaidInventory.takeOne(maid, item)) {
            ItemReporter.markRemoval(BuiltInRegistries.ITEM.getKey(item).toString(), 1);
        }
        maid.swing(InteractionHand.MAIN_HAND);
        run.scaffold.put(pos.immutable(), block);
        run.stepPlaced++;
        return true;
    }

    /**
     * 她已经离开的垫脚块就拆掉：留一地毛刺不像话，生存模式下把方块还回她背包。
     *
     * <p>人还踩在上面、或者正走过去的垫脚块先留着，否则会拆掉自己脚下的台阶。
     */
    private static void cleanScaffold(Run run, EntityMaid maid) {
        if (run.scaffold.isEmpty()) {
            return;
        }
        // 正走过去、正在垫下一块、正在起跳或跑跳时先不拆：拆了脚下的台阶她就上不去了
        if (!run.forceClean
                && (run.walkingTo != null || run.stepTop != null || run.jumpFor != null
                        || run.hop != null)) {
            return;
        }
        AABB body = maid.getBoundingBox().inflate(0.1);
        var iterator = run.scaffold.entrySet().iterator();
        while (iterator.hasNext()) {
            Map.Entry<BlockPos, Block> entry = iterator.next();
            BlockPos pos = entry.getKey();
            BlockState state = run.level.getBlockState(pos);
            if (state.getBlock() != entry.getValue()) {
                iterator.remove();
                continue;
            }
            if (!run.forceClean && body.intersects(new AABB(pos))) {
                continue;
            }
            if (!run.forceClean
                    && ((run.stand != null && pos.distSqr(run.stand) <= 4)
                            || (run.stepTop != null && pos.distSqr(run.stepTop) <= 4))) {
                continue;
            }
            run.level.destroyBlock(pos, false);
            if (run.consumeMaterials) {
                giveBack(run, maid, entry.getValue().asItem());
            }
            iterator.remove();
        }
    }

    /** 把垫脚拆下来的方块塞回她背包；塞不下就掉在她脚下。 */
    private static void giveBack(Run run, EntityMaid maid, Item item) {
        ItemStack stack = new ItemStack(item);
        int count = stack.getCount();
        CombinedInvWrapper backpack = maid.getAvailableBackpackInv();
        for (int slot = 0; slot < backpack.getSlots() && !stack.isEmpty(); slot++) {
            if (slot == MaidInventory.HEADWEAR_SLOT) {
                continue;
            }
            stack = backpack.insertItem(slot, stack, false);
        }
        int inserted = count - stack.getCount();
        if (inserted > 0) {
            // 这块本来就该在她身上：背包差分别把它说成主人刚塞进来的
            ItemReporter.suppressHandGain(new ItemStack(item, inserted));
        }
        if (!stack.isEmpty()) {
            Block.popResource(run.level, maid.blockPosition(), stack);
        }
    }

    /** 收工阶段每 tick 推进：先走去落脚点，路上把垫脚块拆掉，拆干净再真正收尾。 */
    private static void cleaning(MinecraftServer server, Run run, EntityMaid maid) {
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
    private static BlockPos awayStand(Run run, EntityMaid maid) {
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

    private static boolean cleanupStand(Run run, BlockPos stand) {
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

    private static BlockPlaceContext context(EntityMaid maid, Cell cell) {
        return Placement.context(maid, new ItemStack(cell.item), cell.pos, null, null, cell.yaw);
    }

    private static BlockPlaceContext visibleContext(EntityMaid maid, Cell cell) {
        return Placement.visibleContext(
                maid, new ItemStack(cell.item), cell.pos, null, null, cell.yaw);
    }

    private static void markDone(Run run, Cell cell) {
        if (cell.done) {
            return;
        }
        cell.done = true;
        run.built.add(cell.pos);
        run.placed++;
    }

    /** 缺料：这一轮不再为这一格努力，但把还差多少件记下来交给插件。 */
    private static void missing(Run run, Cell cell) {
        if (cell.settled) {
            return;
        }
        cell.settled = true;
        run.missing.merge(cell.itemId, 1, Integer::sum);
    }

    /** 这一遍还不放弃：记下原因，等这一层整轮没有新进展时再一起收。 */
    private static void temporary(Run run, Cell cell, String failure) {
        cell.lastFailure = failure;
    }

    /** 这一格有定论了：记进失败清单，收尾时交给插件。 */
    private static void settle(Run run, Cell cell, String failure) {
        if (cell.done) {
            return;
        }
        boolean already = cell.settled && !cell.deferred;
        cell.settled = true;
        cell.deferred = false;
        if (already) {
            return;
        }
        String code = failure == null || failure.isEmpty() ? "out_of_reach" : failure;
        run.failed.merge(code, 1, Integer::sum);
    }

    /**
     * 这一轮先放一放：别的格子放好之后，缺依托、被挡住的常常就能放了。
     *
     * <p>整张图纸过完一遍再回头补试，补试过还是不行的才由 {@link #settle} 记账。
     */
    private static void defer(Run run, Cell cell, String failure) {
        if (cell.done || cell.settled) {
            return;
        }
        cell.settled = true;
        cell.deferred = true;
        cell.lastFailure = failure == null || failure.isEmpty() ? "out_of_reach" : failure;
    }

    /**
     * 一层走完：这一层还有没盖的、这一遍又有新进展，就再走一遍；连续几遍没有进展
     * 才把这层剩下的格子收进失败清单，换下一层。
     */
    private static void endLayer(MinecraftServer server, Run run) {
        if (run.placed >= run.total) {
            finish(server, run, "");
            return;
        }
        if (!layerHasOpen(run)) {
            if (!moveOn(run) && !retryRound(run)) {
                finish(server, run, "");
                return;
            }
            run.cooldown = PASS_BREAK_TICKS;
            return;
        }
        // 剩下的都在等活物挪开：先歇一会儿，不算这一层失败
        if (openCellsHeld(run)) {
            run.stallPasses = 0;
            run.cursor = run.layerStart;
            run.cooldown = PASS_BREAK_TICKS;
            return;
        }
        // 垫脚也算这一遍的进展：爬上去要好几步，不能爬一半就被当成走不到收掉
        if (run.placed + run.stepPlaced > run.layerPassPlaced) {
            run.layerPassPlaced = run.placed + run.stepPlaced;
            run.stallPasses = 0;
            run.cursor = run.layerStart;
            run.cooldown = PASS_BREAK_TICKS;
            return;
        }
        run.stallPasses++;
        if (run.stallPasses < MAX_STALL_PASSES) {
            run.cursor = run.layerStart;
            run.cooldown = PASS_BREAK_TICKS;
            return;
        }
        if (run.retried) {
            // 补试过了还是不行：这一格才算真有定论
            settleLayer(run);
        } else {
            // 先记着，等整张图纸都过一遍再回来补试
            deferLayer(run);
        }
        if (!moveOn(run) && !retryRound(run)) {
            finish(server, run, "");
            return;
        }
        run.cooldown = PASS_BREAK_TICKS;
    }

    private static boolean layerHasOpen(Run run) {
        boolean doorsOnly = onlyDoorsLeft(run);
        for (int i = run.layerStart; i < run.layerEnd; i++) {
            Cell cell = run.cells.get(i);
            if (cell.done || cell.settled) {
                continue;
            }
            if (!doorsOnly && cell.block == Blocks.OAK_DOOR) {
                continue;
            }
            return true;
        }
        return false;
    }

    /** 这一层剩下的格子是不是全在等活物挪开。 */
    private static boolean openCellsHeld(Run run) {
        boolean doorsOnly = onlyDoorsLeft(run);
        boolean any = false;
        for (int i = run.layerStart; i < run.layerEnd; i++) {
            Cell cell = run.cells.get(i);
            if (cell.done || cell.settled) {
                continue;
            }
            if (!doorsOnly && cell.block == Blocks.OAK_DOOR) {
                continue;
            }
            any = true;
            if (!held(cell, run.level)) {
                return false;
            }
        }
        return any;
    }

    /** 这一层暂时收不起来：先记着，别的层放完之后还会回来补。 */
    private static void deferLayer(Run run) {
        boolean doorsOnly = onlyDoorsLeft(run);
        for (int i = run.layerStart; i < run.layerEnd; i++) {
            Cell cell = run.cells.get(i);
            if (cell.done || cell.settled) {
                continue;
            }
            if (!doorsOnly && cell.block == Blocks.OAK_DOOR) {
                continue;
            }
            defer(run, cell, cell.lastFailure);
        }
    }

    /**
     * 整张图纸都过完了：把之前没放上的格子放回来再试一轮。
     *
     * <p>别的方块放好之后，原来缺依托、被别的方块挡住的格子常常就能放了。每一格只补试
     * 一次：补过还是不行的由 {@link #settle} 记账，不会在同一个地方反复打转。
     */
    private static boolean retryRound(Run run) {
        boolean any = false;
        for (Cell cell : run.cells) {
            if (!cell.done && cell.deferred && !cell.confirmed) {
                any = true;
                break;
            }
        }
        if (!any) {
            return false;
        }
        for (Cell cell : run.cells) {
            if (cell.done || !cell.deferred || cell.confirmed) {
                continue;
            }
            cell.settled = false;
            cell.deferred = false;
            cell.confirmed = true;
            // 周围已经变样了，之前试过的站位不一定还成立
            cell.triedStands.clear();
        }
        run.retried = true;
        return resetLayer(run);
    }

    private static void settleLayer(Run run) {
        boolean doorsOnly = onlyDoorsLeft(run);
        for (int i = run.layerStart; i < run.layerEnd; i++) {
            Cell cell = run.cells.get(i);
            if (cell.done || cell.settled) {
                continue;
            }
            if (!doorsOnly && cell.block == Blocks.OAK_DOOR) {
                continue;
            }
            settle(run, cell, cell.lastFailure);
        }
    }

    /** 换到下一层还有没盖的格子；没有了就回头看门。 */
    private static boolean moveOn(Run run) {
        return advanceLayer(run) || openDoorLayer(run);
    }

    /** 换到下一层还有没盖的格子；没有了返回假。 */
    private static boolean advanceLayer(Run run) {
        boolean doorsOnly = onlyDoorsLeft(run);
        for (int i = run.layerEnd; i < run.cells.size(); i++) {
            Cell cell = run.cells.get(i);
            if (cell.done || cell.settled) {
                continue;
            }
            if (!doorsOnly && cell.block == Blocks.OAK_DOOR) {
                continue;
            }
            setLayer(run, cell.pos.getY());
            return true;
        }
        return false;
    }

    /**
     * 别的格子都处理完了，再回到门所在的那一层把门装上。
     *
     * <p>门在施工期间一直故意留着，不能因为所在层「没有进展」就被当成走不到收掉。
     */
    private static boolean openDoorLayer(Run run) {
        if (!onlyDoorsLeft(run)) {
            return false;
        }
        for (Cell cell : run.cells) {
            if (!cell.done && !cell.settled && cell.block == Blocks.OAK_DOOR) {
                setLayer(run, cell.pos.getY());
                return true;
            }
        }
        return false;
    }

    private static boolean resetLayer(Run run) {
        boolean doorsOnly = onlyDoorsLeft(run);
        for (Cell cell : run.cells) {
            if (cell.done || cell.settled) {
                continue;
            }
            if (!doorsOnly && cell.block == Blocks.OAK_DOOR) {
                continue;
            }
            setLayer(run, cell.pos.getY());
            return true;
        }
        return false;
    }

    private static void setLayer(Run run, int y) {
        int start = 0;
        while (start < run.cells.size() && run.cells.get(start).pos.getY() < y) {
            start++;
        }
        int end = start;
        while (end < run.cells.size() && run.cells.get(end).pos.getY() == y) {
            end++;
        }
        run.layerStart = start;
        run.layerEnd = end;
        run.cursor = start;
        run.layerPassPlaced = run.placed + run.stepPlaced;
        run.stallPasses = 0;
    }

    private static void finish(MinecraftServer server, Run run, String reasonCode) {
        finish(server, run, reasonCode, "");
    }

    /**
     * 收工：还有垫脚块时先走开、拆干净再推收尾事件，别让她停在临时方块上。
     *
     * <p>被人抱走、打晕、被别的指令顶掉时不拖：那几种情况她本来就要离开，
     * 直接按下面的收尾走，能拆的当场拆掉。
     */
    private static void finish(MinecraftServer server, Run run, String reasonCode, String reason) {
        EntityMaid maid = MaidQueries.findMaidById(server, run.maidId);
        boolean natural = reasonCode == null || reasonCode.isEmpty() || "timeout".equals(reasonCode);
        if (natural && !run.cleaning && maid != null && !maid.isRemoved() && !run.scaffold.isEmpty()) {
            run.cleaning = true;
            run.cleanTicks = 0;
            run.pendingCode = reasonCode == null ? "" : reasonCode;
            run.pendingReason = reason == null ? "" : reason;
            run.stand = null;
            run.stepTop = null;
            run.stepFor = null;
            clearJump(run);
            clearHop(run, maid);
            run.cleanStand = awayStand(run, maid);
            if (run.cleanStand != null) {
                MaidActuator.walkToPos(maid, run.cleanStand, 0);
            }
            return;
        }
        finishNow(server, run, reasonCode, reason);
    }

    private static void finishNow(MinecraftServer server, Run run, String reasonCode, String reason) {
        EntityMaid maid = MaidQueries.findMaidById(server, run.maidId);
        if (maid != null) {
            // 中断的路上不再护着脚下那块：垫出来的方块一块都不留
            run.stand = null;
            run.stepTop = null;
            clearJump(run);
            clearHop(run, maid);
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
            if (!cell.done && cell.settled && cell.deferred) {
                settle(run, cell, cell.lastFailure);
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

    private static String naturalReason(Run run) {
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

    private static String reasonText(String code) {
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

    /**
     * 除了门以外的格子都已经放好：这时候才装门，施工期间门洞保持敞开。
     *
     * <p>只看「放好了」，不看「settled」：有格子因为够不着、被挡或液体放弃时，门先不装，
     * 她之后回来补料、接着盖还能从门洞进出；否则半成品把门口封死，人就困在里外了。
     */
    private static boolean onlyDoorsLeft(Run run) {
        for (Cell cell : run.cells) {
            if (cell.done) {
                continue;
            }
            if (cell.block != Blocks.OAK_DOOR) {
                return false;
            }
        }
        return true;
    }

    private static boolean supported(ServerLevel level, BlockPos pos) {
        return solidCell(level, pos.below());
    }

    /** 这一格是不是实心方块：垫脚块要立在它上面，或者它自己就得先垫出来。 */
    private static boolean solidCell(ServerLevel level, BlockPos pos) {
        BlockState state = level.getBlockState(pos);
        return !state.isAir()
                && state.getFluidState().isEmpty()
                && !state.getCollisionShape(level, pos).isEmpty();
    }

    private static boolean arrived(BlockPos stand, EntityMaid maid) {
        return arrived(stand, maid, false);
    }

    /**
     * 到没到落脚点。垫脚时要求她真的站进那一格，差半格不算——
     * 不然下一步会从斜角往对面跳，被旁边的方块挡住后再也上不去。
     */
    private static boolean arrived(BlockPos stand, EntityMaid maid, boolean tight) {
        if (stand == null) {
            return false;
        }
        if (tight) {
            return maid.blockPosition().equals(stand);
        }
        return maid.position().distanceToSqr(Vec3.atBottomCenterOf(stand)) <= AT_STAND
                || maid.blockPosition().distManhattan(stand) <= 1;
    }

    /**
     * 手够不够得着：按原版口径量眼睛到方块外壳的距离，不是到方块中心。
     *
     * <p>中心距离会把「站在旁边、眼睛到中心五格」也算够得着，实际却点不到面。
     */
    private static boolean withinReach(EntityMaid maid, BlockPos target) {
        return new AABB(target).distanceToSqr(maid.getEyePosition())
                <= MineRules.REACH * MineRules.REACH;
    }

    private static boolean selfInside(BlockPos target, EntityMaid maid) {
        return maid.getBoundingBox().intersects(new AABB(target));
    }

    private static boolean otherEntityInside(Run run, BlockPos target, EntityMaid maid) {
        for (Entity entity : run.level.getEntities(maid, new AABB(target))) {
            if (entity.isAlive() && !entity.isSpectator()) {
                return true;
            }
        }
        return false;
    }
}
