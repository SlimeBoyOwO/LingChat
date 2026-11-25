package com.lingchat.maidbridge.debug;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.maid.MaidQueries;
import java.util.ArrayList;
import java.util.EnumSet;
import java.util.List;
import java.util.Set;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.particles.DustParticleOptions;
import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.core.particles.ParticleTypes;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.level.ClipContext;
import net.minecraft.world.level.pathfinder.Path;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.HitResult;
import net.minecraft.world.phys.Vec3;
import org.joml.Vector3f;

/**
 * 施工寻路的粒子可视化：她选的落脚点、此刻走的路、要找的那一格、视线射线、候选清单。
 *
 * <p>只在服务端画，用原版的粒子：谁在附近谁就看得见，不装客户端模组。
 * 整块状态只有一份——同一时间只盯一只女仆，换一只就是换掉这一份。
 * 施工每 tick 把当前阶段交上来（{@link #report}）；选落脚点时逐个候选记一笔结论
 * （{@link #probe}），这一 tick 的候选在画完之后就清掉，不留旧账。
 */
public final class PathDebug {
    /** 施工停了这么久还没新消息，就不再画：收工的最后一帧别一直挂在半空。 */
    private static final int STALE_TICKS = 60;
    /** 一 tick 最多画这么多颗粒子。 */
    private static final int MAX_PARTICLES = 320;
    /** 一 tick 最多记这么多条候选结论。 */
    private static final int MAX_PROBES = 96;
    /** 路线上最多画这么多格：长路按比例抽稀。 */
    private static final int MAX_PATH_NODES = 64;
    /** 自动盯着她时，离玩家多远以内算「附近的那一只」。 */
    private static final double AUTO_RADIUS = 64.0;

    private static final DustParticleOptions RED =
            new DustParticleOptions(new Vector3f(1.0F, 0.2F, 0.15F), 1.0F);
    private static final DustParticleOptions ORANGE =
            new DustParticleOptions(new Vector3f(1.0F, 0.6F, 0.1F), 1.0F);
    private static final DustParticleOptions YELLOW =
            new DustParticleOptions(new Vector3f(1.0F, 0.95F, 0.2F), 1.0F);
    private static final DustParticleOptions GRAY =
            new DustParticleOptions(new Vector3f(0.55F, 0.55F, 0.55F), 1.0F);
    private static final DustParticleOptions BLUE =
            new DustParticleOptions(new Vector3f(0.25F, 0.55F, 1.0F), 0.8F);
    private static final DustParticleOptions CYAN =
            new DustParticleOptions(new Vector3f(0.45F, 0.9F, 1.0F), 0.8F);
    private static final DustParticleOptions GREEN =
            new DustParticleOptions(new Vector3f(0.2F, 1.0F, 0.35F), 1.0F);
    private static final DustParticleOptions LIME =
            new DustParticleOptions(new Vector3f(0.55F, 0.95F, 0.5F), 0.9F);

    /** 能单独开关的几部分画面。指令里按 {@code name} 选。 */
    public enum Layer {
        PATH("path", "路径"),
        STAND("stand", "落脚点"),
        TARGET("target", "目标格"),
        RAY("ray", "视线"),
        SEARCH("search", "搜索范围"),
        PROBE("probe", "候选"),
        HOP("hop", "跑跳"),
        PLACED("place", "下一块");

        private final String name;
        private final String label;

        Layer(String name, String label) {
            this.name = name;
            this.label = label;
        }

        /** 指令与补全里用的名字。 */
        public String id() {
            return name;
        }

        /** 指令里的名字；不认识的给 null。 */
        public static Layer byName(String name) {
            for (Layer layer : values()) {
                if (layer.name.equals(name)) {
                    return layer;
                }
            }
            return null;
        }
    }

    /** 一个候选站位的结论，颜色与它对应。 */
    public enum Verdict {
        /** 从她现在站的地方算过，走不到。 */
        NO_PATH,
        /** 看得见，但走不到。 */
        NO_SIGHT,
        /** 眼睛够不着目标。 */
        OUT_OF_REACH,
        /** 已经试过、或者这一轮跳过的。 */
        SKIPPED,
        /** 寻路能走到，可能被选上。 */
        REACHABLE
    }

    private record Probe(BlockPos target, BlockPos stand, Verdict verdict) {
    }

    private static boolean enabled;
    /** 进存档后自动盯着离玩家最近的她，直到手动 hide。 */
    private static boolean auto;
    private static UUID watched;
    private static String watchedName = "";
    private static long lastReport;
    private static String phase = "";
    private static String targetName = "";
    private static BlockPos stand;
    private static BlockPos target;
    private static int searchRadius;
    private static int searchDown;
    private static int searchUp;
    private static List<BlockPos> hop = List.of();
    private static BlockPos nextPos;
    private static int livePathNodes;
    private static int lastProbeCount;
    private static final List<Probe> probes = new ArrayList<>();
    private static final EnumSet<Layer> layers = EnumSet.allOf(Layer.class);
    private static int drawn;

    private PathDebug() {
    }

    /** 可视化开着没有：调用方拿它挡掉整段调试代码。 */
    public static boolean on() {
        return enabled;
    }

    /** 是不是正在盯这只女仆。 */
    public static boolean watching(EntityMaid maid) {
        return enabled && watched != null && maid != null && watched.equals(maid.getUUID());
    }

    /** 进存档就开始画：自动盯着离玩家最近的那一只，直到手动 hide。 */
    public static void followNearest() {
        auto = true;
        enabled = true;
    }

    /** 自动模式挑一只：各玩家附近最近的那只活着的女仆，选中后全部图层打开。 */
    private static EntityMaid watchNearest(MinecraftServer server) {
        EntityMaid best = null;
        double bestDistance = Double.MAX_VALUE;
        for (ServerPlayer player : server.getPlayerList().getPlayers()) {
            Vec3 at = player.position();
            for (EntityMaid maid : player.level().getEntitiesOfClass(
                    EntityMaid.class, new AABB(at, at).inflate(AUTO_RADIUS), Entity::isAlive)) {
                double distance = maid.distanceToSqr(at);
                if (distance < bestDistance) {
                    bestDistance = distance;
                    best = maid;
                }
            }
        }
        if (best == null) {
            return null;
        }
        clearFrame();
        watched = best.getUUID();
        watchedName = best.getName().getString();
        layers.clear();
        layers.addAll(EnumSet.allOf(Layer.class));
        return best;
    }

    /**
     * 开始盯这只女仆，只显示选中的几部分。
     *
     * <p>不带参数时是全开 / 全关二选一：已经在盯同一只女仆就按当前是「全开」还是
     * 「没全开」翻一下；还没盯她就直接全开。
     */
    public static void show(EntityMaid maid, Set<Layer> selected) {
        boolean same = watching(maid);
        if (!same) {
            enabled = true;
            watched = maid.getUUID();
            watchedName = maid.getName().getString();
            lastReport = 0;
            clearFrame();
            layers.clear();
            layers.addAll(selected.isEmpty() ? EnumSet.allOf(Layer.class) : selected);
            return;
        }
        if (selected.isEmpty()) {
            boolean all = layers.size() >= Layer.values().length;
            layers.clear();
            if (!all) {
                layers.addAll(EnumSet.allOf(Layer.class));
            }
            return;
        }
        layers.clear();
        layers.addAll(selected);
    }

    /** 关掉其中几部分，可视化本身还开着。 */
    public static void hideLayers(Set<Layer> selected) {
        layers.removeAll(selected);
    }

    /** 哪些部分开着，给状态指令用。 */
    public static String layersText() {
        if (!enabled || layers.isEmpty()) {
            return "全部关";
        }
        StringBuilder text = new StringBuilder();
        for (Layer layer : Layer.values()) {
            if (!layers.contains(layer)) {
                continue;
            }
            if (!text.isEmpty()) {
                text.append('、');
            }
            text.append(layer.label);
        }
        return text.toString();
    }

    /** 可选的名字，拼在错误提示里。 */
    public static String layerNames() {
        StringBuilder text = new StringBuilder();
        for (Layer layer : Layer.values()) {
            if (!text.isEmpty()) {
                text.append(' ');
            }
            text.append(layer.id());
        }
        return text.toString();
    }

    /** 关掉可视化；手动关掉就不再自动盯着。 */
    public static void stop() {
        enabled = false;
        auto = false;
        watched = null;
        watchedName = "";
        clearFrame();
        probes.clear();
        layers.clear();
    }

    /** 换存档、停服时清干净。 */
    public static void reset() {
        stop();
    }

    /** 施工每 tick 交一次当前阶段与落脚点。 */
    public static void report(
            EntityMaid maid, String phase, BlockPos stand, BlockPos target,
            List<BlockPos> hopChain, BlockPos nextPlace, String targetBlock) {
        if (!watching(maid)) {
            return;
        }
        PathDebug.phase = phase;
        PathDebug.stand = stand;
        PathDebug.target = target;
        PathDebug.targetName = targetBlock == null ? "" : targetBlock;
        PathDebug.hop = hopChain.isEmpty() ? List.of() : List.copyOf(hopChain);
        PathDebug.nextPos = nextPlace;
        PathDebug.lastReport = maid.level().getGameTime();
    }

    /**
     * 找落脚点时实际扫过的那一圈：蓝色框跟着它走，扩圈时框也跟着变大。
     *
     * <p>只记最后一次搜索的范围：一 tick 里换了几圈，看到的就是最后那一圈。
     */
    public static void searchRange(EntityMaid maid, BlockPos target, int radius, int down, int up) {
        if (!watching(maid) || !layers.contains(Layer.SEARCH)) {
            return;
        }
        searchRadius = radius;
        searchDown = down;
        searchUp = up;
    }

    /** 选落脚点时逐个候选记一笔结论；没开可视化、或者不显示候选时几乎是空的。 */
    public static void probe(EntityMaid maid, BlockPos target, BlockPos stand, Verdict verdict) {
        if (!watching(maid) || !layers.contains(Layer.PROBE) || probes.size() >= MAX_PROBES) {
            return;
        }
        probes.add(new Probe(target.immutable(), stand.immutable(), verdict));
    }

    /** 状态栏文字：指令里直接发出来。 */
    public static String status() {
        if (!enabled) {
            return "寻路可视化：关";
        }
        if (watched == null) {
            return auto ? "寻路可视化：自动开着，附近没找到她" : "寻路可视化：开，但还没选中女仆";
        }
        String who = auto ? "自动盯着 " + watchedName : watchedName;
        if (lastReport == 0) {
            return "寻路可视化：开（" + who + "），她此刻没有在施工";
        }
        return "寻路可视化：" + who
                + "｜阶段：" + phase
                + "｜目标：" + format(target)
                + (targetName.isEmpty() ? "" : "（" + targetName + "）")
                + "｜落脚点：" + format(stand)
                + "｜当前路径：" + (livePathNodes <= 0 ? "没有" : livePathNodes + " 格")
                + "｜上一帧候选：" + lastProbeCount + " 个"
                + "｜显示：" + layersText();
    }

    /** 每 tick 画一次；施工停了就静默，等下一次开工。 */
    public static void tick(MinecraftServer server) {
        if (!enabled) {
            return;
        }
        EntityMaid maid = watched == null ? null : MaidQueries.findMaidById(server, watched.toString());
        if (maid == null || !maid.isAlive()) {
            // 自动模式换一只：她走远到区块卸载、或者刚进存档还没有目标
            maid = auto ? watchNearest(server) : null;
            if (maid == null) {
                watched = null;
                watchedName = "";
                clearFrame();
                probes.clear();
                return;
            }
        }
        if (!(maid.level() instanceof ServerLevel level)) {
            clearFrame();
            probes.clear();
            return;
        }
        long now = level.getGameTime();
        if (lastReport == 0 || now - lastReport > STALE_TICKS) {
            clearFrame();
            probes.clear();
            return;
        }
        drawn = 0;
        // 车万女仆的走路目标走的是 brain：路径通常已经交给寻路器，没有时再从 PATH 记忆里取
        Path path = maid.getNavigation().getPath();
        if (path == null) {
            path = maid.getBrain().getMemory(MemoryModuleType.PATH).orElse(null);
        }
        livePathNodes = path == null ? 0 : path.getNodeCount();
        // 先画最要紧的：粒子有总量上限，排到后面的在极端情况会被丢掉
        if (layers.contains(Layer.PATH)) {
            drawLivePath(level, maid, path);
        }
        if (layers.contains(Layer.TARGET)) {
            drawTarget(level, target);
        }
        if (layers.contains(Layer.RAY)) {
            drawSightRay(level, maid, target);
        }
        if (layers.contains(Layer.STAND)) {
            drawStand(level, stand);
        }
        if (layers.contains(Layer.PLACED) && nextPos != null) {
            wireBox(level, ParticleTypes.SOUL_FIRE_FLAME, nextPos, nextPos, 0.5);
        }
        if (layers.contains(Layer.PROBE)) {
            drawProbes(level);
        }
        if (layers.contains(Layer.HOP)) {
            drawHop(level, hop);
        }
        if (layers.contains(Layer.SEARCH)) {
            drawSearchBox(level, target);
        }
        lastProbeCount = probes.size();
        probes.clear();
    }

    private static void clearFrame() {
        phase = "";
        targetName = "";
        stand = null;
        target = null;
        searchRadius = 0;
        searchDown = 0;
        searchUp = 0;
        hop = List.of();
        nextPos = null;
        livePathNodes = 0;
        lastProbeCount = 0;
        lastReport = 0;
    }

    private static String format(BlockPos pos) {
        return pos == null ? "—" : pos.getX() + ", " + pos.getY() + ", " + pos.getZ();
    }

    /** 落脚点的候选范围：由找站位那一层报上来，扩圈时框跟着变大，始终套在当前目标外面。 */
    private static void drawSearchBox(ServerLevel level, BlockPos target) {
        if (target == null) {
            return;
        }
        int radius = searchRadius > 0 ? searchRadius : 4;
        int down = searchDown > 0 ? searchDown : 6;
        int up = searchUp > 0 ? searchUp : 3;
        wireBox(level, BLUE, target.offset(-radius, -down, -radius),
                target.offset(radius, up, radius), 1.0);
    }

    /** 这一 tick 搜过的候选：红走不到、橙看不见、黄够不着、灰跳过、浅绿能走。 */
    private static void drawProbes(ServerLevel level) {
        for (Probe probe : probes) {
            DustParticleOptions color = switch (probe.verdict()) {
                case NO_PATH -> RED;
                case NO_SIGHT -> ORANGE;
                case OUT_OF_REACH -> YELLOW;
                case SKIPPED -> GRAY;
                case REACHABLE -> LIME;
            };
            dot(level, color, probe.stand().getX() + 0.5,
                    probe.stand().getY() + 0.12, probe.stand().getZ() + 0.5);
        }
    }

    /** 要找的那一格：八个角上一圈白点，上面再立一根可见的柱子。 */
    private static void drawTarget(ServerLevel level, BlockPos target) {
        if (target == null) {
            return;
        }
        double x = target.getX();
        double y = target.getY();
        double z = target.getZ();
        for (int dx = 0; dx <= 1; dx++) {
            for (int dy = 0; dy <= 1; dy++) {
                for (int dz = 0; dz <= 1; dz++) {
                    dot(level, ParticleTypes.END_ROD, x + dx, y + dy, z + dz);
                }
            }
        }
        line(level, ParticleTypes.END_ROD,
                new Vec3(x + 0.5, y + 1.0, z + 0.5), new Vec3(x + 0.5, y + 3.2, z + 0.5), 8, 0.4);
    }

    /** 选定的落脚点：绿框出她脚下要站的两格高。 */
    private static void drawStand(ServerLevel level, BlockPos stand) {
        if (stand == null) {
            return;
        }
        double x = stand.getX();
        double y = stand.getY();
        double z = stand.getZ();
        for (int dx = 0; dx <= 1; dx++) {
            for (int dz = 0; dz <= 1; dz++) {
                for (int dy = 0; dy <= 2; dy++) {
                    dot(level, GREEN, x + dx, y + dy, z + dz);
                }
            }
        }
    }

    /** 跑跳路线：每个落点一团火，落点之间连起来。 */
    private static void drawHop(ServerLevel level, List<BlockPos> chain) {
        Vec3 previous = null;
        for (BlockPos landing : chain) {
            Vec3 feet = Vec3.atBottomCenterOf(landing);
            dot(level, ParticleTypes.FLAME, feet.x, feet.y + 0.15, feet.z);
            if (previous != null) {
                line(level, ORANGE, previous.add(0.0, 0.9, 0.0), feet.add(0.0, 0.9, 0.0), 12, 0.35);
            }
            previous = feet;
        }
    }

    /** 她此刻真正在走的 A* 路径：每个节点一颗 glow，终点加一圈绿。 */
    private static void drawLivePath(ServerLevel level, EntityMaid maid, Path path) {
        if (path == null || path.getNodeCount() <= 0) {
            return;
        }
        int count = Math.min(path.getNodeCount(), MAX_PATH_NODES);
        for (int i = 0; i < count; i++) {
            BlockPos node = path.getNodePos(i);
            dot(level, ParticleTypes.GLOW,
                    node.getX() + 0.5, node.getY() + 0.25, node.getZ() + 0.5);
        }
        BlockPos end = path.getNodePos(count - 1);
        dot(level, GREEN, end.getX() + 0.5, end.getY() + 0.5, end.getZ() + 0.5);
        dot(level, ParticleTypes.HAPPY_VILLAGER,
                maid.getX(), maid.getY() + 2.3, maid.getZ());
    }

    /**
     * 眼睛到目标格中心的射线。看得见是淡青蓝的点；半路被挡住的，挡路的方块上画一团红。
     */
    private static void drawSightRay(ServerLevel level, EntityMaid maid, BlockPos target) {
        if (target == null) {
            return;
        }
        Vec3 eye = maid.getEyePosition();
        Vec3 center = Vec3.atCenterOf(target);
        if (eye.distanceToSqr(center) < 0.01) {
            return;
        }
        BlockHitResult hit = level.clip(new ClipContext(
                eye, center, ClipContext.Block.COLLIDER, ClipContext.Fluid.NONE, maid));
        boolean blocked = hit.getType() == HitResult.Type.BLOCK
                && !hit.getBlockPos().equals(target)
                && hit.getLocation().distanceToSqr(eye) < center.distanceToSqr(eye) - 1.0E-3;
        line(level, blocked ? RED : CYAN, eye,
                blocked ? hit.getLocation() : center, 32, 0.4);
        if (blocked) {
            BlockPos wall = hit.getBlockPos();
            dot(level, RED, wall.getX() + 0.5, wall.getY() + 0.5, wall.getZ() + 0.5);
        }
    }

    /** 两格之间的稀疏点线。 */
    private static void line(
            ServerLevel level, ParticleOptions particle, Vec3 from, Vec3 to, int max, double gap) {
        double length = from.distanceTo(to);
        if (length < 1.0E-3) {
            return;
        }
        int steps = (int) Math.ceil(length / gap);
        steps = Math.max(1, Math.min(steps, max));
        for (int i = 1; i <= steps; i++) {
            Vec3 at = from.lerp(to, (double) i / steps);
            dot(level, particle, at.x, at.y, at.z);
        }
    }

    /** 长方体线框，步长按格算。 */
    private static void wireBox(
            ServerLevel level, ParticleOptions particle, BlockPos from, BlockPos to, double gap) {
        double x0 = from.getX();
        double y0 = from.getY();
        double z0 = from.getZ();
        double x1 = to.getX() + 1.0;
        double y1 = to.getY() + 1.0;
        double z1 = to.getZ() + 1.0;
        Vec3[] corners = new Vec3[] {
            new Vec3(x0, y0, z0), new Vec3(x1, y0, z0),
            new Vec3(x0, y0, z1), new Vec3(x1, y0, z1),
            new Vec3(x0, y1, z0), new Vec3(x1, y1, z0),
            new Vec3(x0, y1, z1), new Vec3(x1, y1, z1)
        };
        int[][] edges = {
            {0, 1}, {2, 3}, {4, 5}, {6, 7},
            {0, 2}, {1, 3}, {4, 6}, {5, 7},
            {0, 4}, {1, 5}, {2, 6}, {3, 7}
        };
        for (int[] edge : edges) {
            line(level, particle, corners[edge[0]], corners[edge[1]], 16, gap);
        }
    }

    private static void dot(ServerLevel level, ParticleOptions particle, double x, double y, double z) {
        if (drawn >= MAX_PARTICLES) {
            return;
        }
        drawn++;
        level.sendParticles(particle, x, y, z, 1, 0.0, 0.0, 0.0, 0.0);
    }
}
