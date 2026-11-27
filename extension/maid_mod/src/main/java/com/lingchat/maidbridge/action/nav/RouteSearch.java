package com.lingchat.maidbridge.action.nav;

import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.PriorityQueue;
import java.util.Set;
import net.minecraft.core.BlockPos;

/**
 * 落脚点图上的 A*：节点是站得住的格子，边是走、跳下、跑跳、搭一格、垫一格、拆一格。
 *
 * <p>以前先问原版寻路「走不走得到」，算不出路径的站位直接出局，再由几条写死的兜底
 * 分支从头猜一遍；能不能到取决于原版寻路认不认这条路。现在把几种手段都当成边，由搜索
 * 自己组合路线，代价高的手段排在后面，搜不到才算走不通。
 *
 * <p>只算路线，不碰世界：世界怎么读由调用方实现的 {@link View} 决定，所以能脱开
 * Minecraft 跑单测。路线怎么执行交给 {@link Navigator} 的几条腿。
 */
final class RouteSearch {
    /** 一次调用最多展开多少个节点：算不完留到下个 tick，别把服务端卡住。 */
    private static final int EXPAND_PER_CALL = 400;
    /** 一次搜索至少要允许这么多节点：小盒子里的正常路线不会被额度卡住。 */
    private static final int MIN_NODES = 6000;
    /** 盒子再大也不超过这么多节点：到顶只收达标候选，不再往外长。 */
    private static final int MAX_NODES = 20000;

    /** 各种手段的相对代价：能走就别跳，能跳就别动方块。 */
    private static final double DIAGONAL_COST = 1.4142;
    private static final double HOP_COST = 4.0;
    private static final double DROP_COST = 3.0;
    private static final double BRIDGE_COST = 7.0;
    private static final double RISE_COST = 7.0;
    private static final double RISE_SUPPORT_COST = 11.0;
    /** 正上方直接垫柱子，比往旁边垫一格便宜。 */
    private static final double PILLAR_COST = 6.0;
    /** 拆一块挡路的方块：比放一块贵得多，只在别的办法都不行时才走。 */
    private static final double BREAK_COST = 20.0;

    /** 八个水平方向；前四个是正的上下左右，后四个是斜角。 */
    private static final int[][] SIDES = {
        {1, 0}, {-1, 0}, {0, 1}, {0, -1}, {1, 1}, {1, -1}, {-1, 1}, {-1, -1}
    };

    /** 一条边对应的一次动作；有没有方块、要不要先垫托底都记在这里。 */
    record Step(Kind kind, BlockPos from, BlockPos to, BlockPos block, boolean support) {
        static Step walk(BlockPos from, BlockPos to) {
            return new Step(Kind.WALK, from, to, null, false);
        }
    }

    /** 手段。走是基本单位，其余几种会改世界或者把她摆过去。 */
    enum Kind {
        WALK,
        HOP,
        DROP,
        BRIDGE,
        RISE,
        PILLAR,
        BREAK
    }

    /** 搜索的结果：还在算、算到了、搜完了没有。 */
    enum State {
        SEARCHING,
        FOUND,
        EXHAUSTED
    }

    /** 这一次目标还剩多少额度可以平铺：垫脚与升高不限，只有搭路按格数扣。 */
    record Limits(int bridges) {
    }

    /**
     * 世界的样子：搜索只通过这些判断决定能不能走、能不能放。
     *
     * <p>全部按「她站到这一格之后」来判，不看此刻她本人在哪。
     */
    interface View {
        /** 站得住：脚与头是空的，脚下实心。 */
        boolean standable(BlockPos pos);

        /** 能放方块：空气或可替换，不是液体。 */
        boolean air(BlockPos pos);

        /** 实心方块：垫脚块要立在它上面。 */
        boolean solid(BlockPos pos);

        /** 能走过去：站得住，或者是一扇她推得开的木门。 */
        boolean passable(BlockPos pos);

        /** 站在 from 能把这格垫脚块放下：政策允许、没有活物、够得着又看得见。 */
        boolean placeable(BlockPos block, BlockPos from);

        /** 站到 stand 之后身体不压着 target。 */
        boolean fits(BlockPos stand, BlockPos target);

        /** 这一跳能从 from 落到 to；builtFloor 为真时 from 脚下是路线上还没放下的垫脚块。 */
        boolean hop(BlockPos from, BlockPos to, boolean builtFloor);

        /** 这一下能从 from 直落 to（深落，摔伤已免）；builtFloor 同上。 */
        boolean drop(BlockPos from, BlockPos to, boolean builtFloor);

        /** 这一格能拆掉开路：策略允许、拆得动。 */
        boolean breakable(BlockPos pos);

        /** 站到这一格就能动手。 */
        boolean goal(BlockPos pos);
    }

    private final View view;
    private final BlockPos start;
    private final BlockPos target;
    private final Limits limits;
    /** 搜索盒子往外扩了几格：盒子就是起点到目标包围盒再外扩这一圈。 */
    private final int pad;
    private final BlockPos boxMin;
    private final BlockPos boxMax;
    /** 这一次搜索允许生成的节点数：随盒子体积放宽，最宽不超过 MAX_NODES。 */
    private final int maxNodes;
    /** 这一档手段放不放平铺搭路、放不放拆方块。 */
    private final boolean allowBridges;
    private final boolean allowBreaks;

    private final PriorityQueue<Node> open = new PriorityQueue<>(Comparator
            .comparingDouble((Node node) -> node.f)
            .thenComparingLong(node -> node.seq));
    private final Map<BlockPos, Node> best = new HashMap<>();
    private final Set<BlockPos> closed = new HashSet<>();

    private Node found;
    private boolean exhausted;
    /** 额度用尽、有候选被挡在外面：搜空不代表真的没有路。 */
    private boolean truncated;
    private int generated;
    private long seq;

    private RouteSearch(View view, BlockPos start, BlockPos target, Limits limits, int pad,
            boolean allowBridges, boolean allowBreaks) {
        this.view = view;
        this.start = start.immutable();
        this.target = target.immutable();
        this.limits = limits;
        this.pad = Math.max(0, pad);
        this.allowBridges = allowBridges;
        this.allowBreaks = allowBreaks;
        int minX = Math.min(this.start.getX(), this.target.getX()) - this.pad;
        int minY = Math.min(this.start.getY(), this.target.getY()) - this.pad;
        int minZ = Math.min(this.start.getZ(), this.target.getZ()) - this.pad;
        int maxX = Math.max(this.start.getX(), this.target.getX()) + this.pad;
        int maxY = Math.max(this.start.getY(), this.target.getY()) + this.pad;
        int maxZ = Math.max(this.start.getZ(), this.target.getZ()) + this.pad;
        this.boxMin = new BlockPos(minX, minY, minZ);
        this.boxMax = new BlockPos(maxX, maxY, maxZ);
        long volume = (long) (maxX - minX + 1) * (maxY - minY + 1) * (maxZ - minZ + 1);
        this.maxNodes = (int) Math.min(MAX_NODES, Math.max(MIN_NODES, volume * 3));
        Node root = new Node(this.start, null, null, null, false, 0.0, heuristic(this.start),
                0, ++seq);
        open.add(root);
        best.put(this.start, root);
    }

    /**
     * 开一次搜索。
     *
     * @param pad 搜索盒子相对起点与目标包围盒的外扩格数；盒子外的格子不生成节点
     * @param allowBridges 这一档放不放平铺搭路
     * @param allowBreaks 这一档放不放拆掉挡路的方块
     */
    static RouteSearch start(View view, BlockPos start, BlockPos target, Limits limits, int pad,
            boolean allowBridges, boolean allowBreaks) {
        return new RouteSearch(
                view, start, target, limits, pad, allowBridges, allowBreaks);
    }

    /** 平铺与破坏都放开的搜索：单测与兜底用。 */
    static RouteSearch start(
            View view, BlockPos start, BlockPos target, Limits limits, int pad) {
        return start(view, start, target, limits, pad, true, true);
    }

    /** 算一小段，返回这次调用之后的进展。 */
    State step() {
        if (found != null) {
            return State.FOUND;
        }
        if (exhausted) {
            return State.EXHAUSTED;
        }
        int budget = EXPAND_PER_CALL;
        while (budget-- > 0) {
            Node node = open.poll();
            if (node == null) {
                exhausted = true;
                return State.EXHAUSTED;
            }
            if (!closed.add(node.pos)) {
                continue;
            }
            if (node.parent != null && stands(node) && view.goal(node.pos)) {
                found = node;
                return State.FOUND;
            }
            neighbors(node);
            // 额度边界上收到的达标候选要立刻交出去，别等下一轮再从队列里看
            if (found != null) {
                return State.FOUND;
            }
        }
        return State.SEARCHING;
    }

    /** 这次搜索是不是被自己的节点额度截断过：截断后的搜空不代表没有路。 */
    boolean truncated() {
        return truncated;
    }

    /** 这次搜索的盒子往外扩了几格，给日志看。 */
    int pad() {
        return pad;
    }

    /**
     * 调试用的一句话：一共长出多少节点、离目标最近到过哪一格。
     *
     * <p>搜空时调用方把这句话报出来：最近的一格离目标很远，说明卡在她这一片走不出去；
     * 就在目标旁边，说明是目标那几格站不进去。
     */
    String summary() {
        BlockPos nearest = null;
        double nearestDistance = Double.MAX_VALUE;
        for (BlockPos pos : best.keySet()) {
            double distance = pos.distSqr(target);
            if (distance < nearestDistance) {
                nearestDistance = distance;
                nearest = pos;
            }
        }
        String at = nearest == null
                ? "无"
                : nearest.getX() + "," + nearest.getY() + "," + nearest.getZ()
                        + "（" + String.format(Locale.ROOT, "%.1f", Math.sqrt(nearestDistance)) + " 格）";
        return "节点 " + generated + "，闭 " + closed.size() + "，最近 " + at
                + "，盒 ±" + pad + (truncated ? "，截断" : "");
    }

    /** 算到的路线；还没算到给空表。 */
    List<Step> route() {
        if (found == null) {
            return List.of();
        }
        List<Node> chain = new ArrayList<>();
        for (Node node = found; node != null; node = node.parent) {
            chain.add(node);
        }
        Collections.reverse(chain);
        List<Step> steps = new ArrayList<>();
        Node run = chain.get(0);
        for (int i = 1; i < chain.size(); i++) {
            Node node = chain.get(i);
            if (node.kind == Kind.WALK) {
                continue;
            }
            // 连续走的几段合成一次寻路，交给原版导航一次走完
            if (!node.parent.pos.equals(run.pos)) {
                steps.add(Step.walk(run.pos, node.parent.pos));
            }
            steps.add(new Step(node.kind, node.parent.pos, node.pos, node.block, node.support));
            run = node;
        }
        Node last = chain.get(chain.size() - 1);
        if (!last.pos.equals(run.pos)) {
            steps.add(Step.walk(run.pos, last.pos));
        }
        return steps;
    }

    /** 这一格站不站得住：现成能站的，或者这一步自己刚铺出脚下的方块。 */
    private boolean stands(Node node) {
        return builtFloor(node)
                || node.kind == Kind.BREAK
                || view.standable(node.pos);
    }

    /** 脚下是这条路线自己垫出来的方块：世界里还没有，但走到这一步时它已经在了。 */
    private static boolean builtFloor(Node node) {
        return node.kind == Kind.BRIDGE || node.kind == Kind.RISE || node.kind == Kind.PILLAR;
    }

    private void neighbors(Node node) {
        BlockPos here = node.pos;
        boolean edge = false;
        for (int side = 0; side < SIDES.length; side++) {
            int dx = SIDES[side][0];
            int dz = SIDES[side][1];
            boolean diagonal = dx != 0 && dz != 0;
            if (diagonal && !(view.passable(here.offset(dx, 0, 0))
                    && view.passable(here.offset(0, 0, dz)))) {
                // 斜着穿墙角不算路：两边都站得住，才算走得过去
                edge = true;
                continue;
            }
            double cost = diagonal ? DIAGONAL_COST : 1.0;
            boolean level = false;
            for (int dy = -ParkourRules.MAX_FALL; dy <= 1; dy++) {
                BlockPos to = here.offset(dx, dy, dz);
                if (!view.passable(to)) {
                    continue;
                }
                // 抬高一步要跳：头顶两格都得让得开
                if (dy == 1 && !view.air(here.above(2))) {
                    continue;
                }
                level |= dy >= 0;
                add(node, to, Kind.WALK, null, false, cost, 0);
            }
            // 平着或抬一格走得过去才不必动别的手段；往下走得到的坑底、低处地面照常算路，
            // 但不再拦着跨过去或铺过去
            if (level) {
                continue;
            }
            edge = true;
            if (diagonal) {
                continue;
            }
            drop(node, dx, dz);
            rise(node, dx, dz);
            if (allowBridges) {
                bridge(node, dx, dz);
            }
            if (allowBreaks) {
                breakThrough(node, dx, dz);
            }
        }
        // 四周平地连成片时不必试跑跳；有一侧断了才在一圈落点里找
        if (edge) {
            hops(node);
        }
        pillar(node);
    }

    /** 跑跳跨缺口：正向、斜向、错位的落点都试，飞行包络由视图去验。 */
    private void hops(Node node) {
        boolean built = builtFloor(node);
        int reach = ParkourRules.MAX_DISTANCE;
        for (int dx = -reach; dx <= reach; dx++) {
            for (int dz = -reach; dz <= reach; dz++) {
                for (int rise = ParkourRules.MAX_RISE; rise >= -ParkourRules.MAX_FALL; rise--) {
                    if (!ParkourRules.allowed(dx, dz, rise, ParkourRules.MAX_FALL)) {
                        continue;
                    }
                    BlockPos to = node.pos.offset(dx, rise, dz);
                    if (inBox(to) && view.hop(node.pos, to, built)) {
                        add(node, to, Kind.HOP, null, false, HOP_COST, 0);
                    }
                }
            }
        }
    }

    /** 从高处直接落到下面：建筑模式免摔伤，比绕着垫便宜。 */
    private void drop(Node node, int dx, int dz) {
        for (int distance = 1; distance <= ParkourRules.MAX_DISTANCE; distance++) {
            for (int fall = -ParkourRules.MAX_FALL - 1; fall >= -ParkourRules.DEEP_FALL; fall--) {
                BlockPos to = node.pos.offset(dx * distance, fall, dz * distance);
                if (view.drop(node.pos, to, builtFloor(node))) {
                    add(node, to, Kind.DROP, null, false, DROP_COST, 0);
                }
            }
        }
    }

    /** 前方悬空：在脚下铺一格，平着走过去。 */
    private void bridge(Node node, int dx, int dz) {
        BlockPos ahead = node.pos.offset(dx, 0, dz);
        if (ahead.equals(target)
                || view.standable(ahead)
                || !view.air(ahead)
                || !view.air(ahead.above())
                || !view.air(ahead.below())
                || !view.placeable(ahead.below(), node.pos)) {
            return;
        }
        add(node, ahead, Kind.BRIDGE, ahead.below(), false, BRIDGE_COST, 1);
    }

    /** 旁边垫一格再站上去；垫之前脚下没依托就先补一块托底。 */
    private void rise(Node node, int dx, int dz) {
        BlockPos here = node.pos;
        if (target.getY() < here.getY()) {
            return;
        }
        BlockPos top = here.offset(dx, 0, dz);
        BlockPos to = top.above();
        if (top.equals(target)
                || to.equals(target)
                || !view.air(top)
                || !view.air(to)
                || !view.air(to.above())
                || !view.fits(to, target)) {
            return;
        }
        if (!view.solid(top.below())) {
            if (!view.air(top.below()) || !view.placeable(top.below(), here)) {
                return;
            }
            add(node, to, Kind.RISE, top, true, RISE_SUPPORT_COST, 0);
            return;
        }
        if (!view.placeable(top, here)) {
            return;
        }
        add(node, to, Kind.RISE, top, false, RISE_COST, 0);
    }

    /** 目标就在正上方：原地跳起来把脚下补上，落下来就高了一格；垫出来的柱子可以接着垫。 */
    private void pillar(Node node) {
        BlockPos here = node.pos;
        BlockPos to = here.above();
        if (target.getY() <= here.getY()
                || to.equals(target)
                || !view.air(here)
                || !view.air(here.above(2))
                || !(builtFloor(node) || view.solid(here.below()))
                || !view.fits(to, target)
                || !view.placeable(here, here)) {
            return;
        }
        add(node, to, Kind.PILLAR, here, false, PILLAR_COST, 0);
    }

    /**
     * 前面被实心方块挡死：拆掉脚和头两格再走过去。
     *
     * <p>只拆这一格与它头顶那一格，脚下那格要留着当楼板；目标格本身不许拆，那是要动手
     * 的地方。
     */
    private void breakThrough(Node node, int dx, int dz) {
        BlockPos to = node.pos.offset(dx, 0, dz);
        if (to.equals(target) || !view.solid(to) || !view.solid(to.below())) {
            return;
        }
        BlockPos head = to.above();
        if (!view.breakable(to)) {
            return;
        }
        if (!view.air(head) && !view.breakable(head)) {
            return;
        }
        int blocks = view.air(head) ? 1 : 2;
        add(node, to, Kind.BREAK, null, false, BREAK_COST * blocks, 0);
    }

    private void add(Node from, BlockPos to, Kind kind, BlockPos block, boolean support,
            double cost, int bridges) {
        // 盒子外的格子根本不生成：搜索一开始只跑起点到目标这一带
        if (!inBox(to)) {
            return;
        }
        int usedBridges = from.bridges + bridges;
        if (usedBridges > limits.bridges()) {
            return;
        }
        double g = from.g + cost;
        if (generated >= maxNodes) {
            // 额度用尽就不再往外长，但「已经站在到位线里、站得住」的候选照收：
            // 否则最后一步会被额度吃掉，搜空就被误当成没有路
            truncated = true;
            if (isGoal(kind, to)) {
                found = new Node(to, from, kind, block, support, g, g + heuristic(to),
                        usedBridges, ++seq);
            }
            return;
        }
        Node old = best.get(to);
        if (old != null && old.g <= g + 1.0E-6) {
            return;
        }
        Node node = new Node(to, from, kind, block, support, g, g + heuristic(to),
                usedBridges, ++seq);
        best.put(to, node);
        open.add(node);
        generated++;
    }

    private boolean inBox(BlockPos pos) {
        return pos.getX() >= boxMin.getX() && pos.getX() <= boxMax.getX()
                && pos.getY() >= boxMin.getY() && pos.getY() <= boxMax.getY()
                && pos.getZ() >= boxMin.getZ() && pos.getZ() <= boxMax.getZ();
    }

    /** 这一格站不站得住、到没到目标：只给额度边界上的收尾判断用。 */
    private boolean isGoal(Kind kind, BlockPos pos) {
        boolean stands = kind == Kind.BRIDGE
                || kind == Kind.RISE
                || kind == Kind.PILLAR
                || kind == Kind.BREAK
                || view.standable(pos);
        return stands && view.goal(pos);
    }

    /** 到目标的直线距离：走的代价是每格 1，这个估计不会高过实际代价。 */
    private double heuristic(BlockPos pos) {
        return Math.sqrt(pos.distSqr(target));
    }

    private static final class Node {
        final BlockPos pos;
        final Node parent;
        final Kind kind;
        final BlockPos block;
        final boolean support;
        final double g;
        final double f;
        final int bridges;
        final long seq;

        Node(BlockPos pos, Node parent, Kind kind, BlockPos block, boolean support,
                double g, double f, int bridges, long seq) {
            this.pos = pos;
            this.parent = parent;
            this.kind = kind;
            this.block = block;
            this.support = support;
            this.g = g;
            this.f = f;
            this.bridges = bridges;
            this.seq = seq;
        }
    }
}
