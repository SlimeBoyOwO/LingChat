package com.lingchat.maidbridge.action.nav;

import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.PriorityQueue;
import java.util.Set;
import net.minecraft.core.BlockPos;

/**
 * 落脚点图上的 A*：节点是站得住的格子，边是走、跳下、跑跳、搭一格、垫一格。
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
    /** 整次搜索最多生成多少个节点；到顶就认作搜不到，不做无限展开。 */
    private static final int MAX_NODES = 6000;
    /** 离目标这么近才开始试垫脚与搭路：远处的缺口先交给走和跑跳。 */
    private static final int NEAR = 16;

    /** 各种手段的相对代价：能走就别跳，能跳就别动方块。 */
    private static final double DIAGONAL_COST = 1.4142;
    private static final double HOP_COST = 4.0;
    private static final double DROP_COST = 3.0;
    private static final double BRIDGE_COST = 7.0;
    private static final double RISE_COST = 7.0;
    private static final double RISE_SUPPORT_COST = 11.0;
    private static final double PILLAR_COST = 8.0;

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
        PILLAR
    }

    /** 搜索的结果：还在算、算到了、搜完了没有。 */
    enum State {
        SEARCHING,
        FOUND,
        EXHAUSTED
    }

    /** 这一次目标还剩多少额度可以动地形：垫脚块总数、升高次数、平铺块数、深落次数。 */
    record Limits(int blocks, int steps, int bridges, int drops) {
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

        /** 这一跳能从 from 落到 to。 */
        boolean hop(BlockPos from, BlockPos to);

        /** 这一下能从 from 直落 to（深落，摔伤已免）。 */
        boolean drop(BlockPos from, BlockPos to);

        /** 站到这一格就能动手。 */
        boolean goal(BlockPos pos);
    }

    private final View view;
    private final BlockPos start;
    private final BlockPos target;
    private final Limits limits;

    private final PriorityQueue<Node> open = new PriorityQueue<>(Comparator
            .comparingDouble((Node node) -> node.f)
            .thenComparingLong(node -> node.seq));
    private final Map<BlockPos, Node> best = new HashMap<>();
    private final Set<BlockPos> closed = new HashSet<>();

    private Node found;
    private boolean exhausted;
    private int generated;
    private long seq;

    private RouteSearch(View view, BlockPos start, BlockPos target, Limits limits) {
        this.view = view;
        this.start = start.immutable();
        this.target = target.immutable();
        this.limits = limits;
        Node root = new Node(this.start, null, null, null, false, 0.0, heuristic(this.start),
                0, 0, 0, 0, ++seq);
        open.add(root);
        best.put(this.start, root);
    }

    static RouteSearch start(View view, BlockPos start, BlockPos target, Limits limits) {
        return new RouteSearch(view, start, target, limits);
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
        }
        return State.SEARCHING;
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
        return node.kind == Kind.BRIDGE
                || node.kind == Kind.RISE
                || node.kind == Kind.PILLAR
                || view.standable(node.pos);
    }

    private void neighbors(Node node) {
        BlockPos here = node.pos;
        for (int side = 0; side < SIDES.length; side++) {
            int dx = SIDES[side][0];
            int dz = SIDES[side][1];
            boolean diagonal = dx != 0 && dz != 0;
            if (diagonal && !(view.passable(here.offset(dx, 0, 0))
                    && view.passable(here.offset(0, 0, dz)))) {
                // 斜着穿墙角不算路：两边都站得住，才算走得过去
                continue;
            }
            double cost = diagonal ? DIAGONAL_COST : 1.0;
            boolean walked = false;
            for (int dy = -ParkourRules.MAX_FALL; dy <= 1; dy++) {
                BlockPos to = here.offset(dx, dy, dz);
                if (!view.passable(to)) {
                    continue;
                }
                // 抬高一步要跳：头顶两格都得让得开
                if (dy == 1 && !view.air(here.above(2))) {
                    continue;
                }
                walked = true;
                add(node, to, Kind.WALK, null, false, cost, 0, 0, 0, 0);
            }
            if (walked || diagonal) {
                continue;
            }
            // 这个方向走不过去：跑跳、直落、搭一格、垫一格，谁便宜用谁
            hop(node, dx, dz);
            drop(node, dx, dz);
            if (!near(here)) {
                continue;
            }
            bridge(node, dx, dz);
            rise(node, dx, dz);
        }
        if (near(here)) {
            pillar(node);
        }
    }

    /** 跑跳跨缺口。 */
    private void hop(Node node, int dx, int dz) {
        for (int distance = 2; distance <= ParkourRules.MAX_DISTANCE; distance++) {
            for (int rise = ParkourRules.MAX_RISE; rise >= -ParkourRules.MAX_FALL; rise--) {
                if (!ParkourRules.allowed(distance, rise)) {
                    continue;
                }
                BlockPos to = node.pos.offset(dx * distance, rise, dz * distance);
                if (view.hop(node.pos, to)) {
                    add(node, to, Kind.HOP, null, false, HOP_COST, 0, 0, 0, 0);
                }
            }
        }
    }

    /** 从高处直接落到下面：建筑模式免摔伤，比绕着垫便宜。 */
    private void drop(Node node, int dx, int dz) {
        for (int distance = 1; distance <= ParkourRules.MAX_DISTANCE; distance++) {
            for (int fall = -ParkourRules.MAX_FALL - 1; fall >= -ParkourRules.DEEP_FALL; fall--) {
                BlockPos to = node.pos.offset(dx * distance, fall, dz * distance);
                if (view.drop(node.pos, to)) {
                    add(node, to, Kind.DROP, null, false, DROP_COST, 0, 0, 1, 0);
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
        add(node, ahead, Kind.BRIDGE, ahead.below(), false, BRIDGE_COST, 1, 0, 1, 0);
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
            add(node, to, Kind.RISE, top, true, RISE_SUPPORT_COST, 2, 1, 0, 0);
            return;
        }
        if (!view.placeable(top, here)) {
            return;
        }
        add(node, to, Kind.RISE, top, false, RISE_COST, 1, 1, 0, 0);
    }

    /** 目标就在正上方：原地跳起来把脚下补上，落下来就高了一格。 */
    private void pillar(Node node) {
        BlockPos here = node.pos;
        BlockPos to = here.above();
        if (target.getY() <= here.getY()
                || to.equals(target)
                || !view.air(here)
                || !view.air(here.above(2))
                || !view.solid(here.below())
                || !view.fits(to, target)
                || !view.placeable(here, here)) {
            return;
        }
        add(node, to, Kind.PILLAR, here, false, PILLAR_COST, 1, 1, 0, 0);
    }

    private void add(Node from, BlockPos to, Kind kind, BlockPos block, boolean support,
            double cost, int blocks, int steps, int bridges, int drops) {
        // 节点额度用完就不再往外长：已经在队列里的还能继续收，收完自然判搜不到
        if (generated >= MAX_NODES) {
            return;
        }
        int usedBlocks = from.blocks + blocks;
        int usedSteps = from.steps + steps;
        int usedBridges = from.bridges + bridges;
        int usedDrops = from.drops + drops;
        if (usedBlocks > limits.blocks()
                || usedSteps > limits.steps()
                || usedBridges > limits.bridges()
                || usedDrops > limits.drops()) {
            return;
        }
        double g = from.g + cost;
        Node old = best.get(to);
        if (old != null && old.g <= g + 1.0E-6) {
            return;
        }
        Node node = new Node(to, from, kind, block, support, g, g + heuristic(to),
                usedBlocks, usedSteps, usedBridges, usedDrops, ++seq);
        best.put(to, node);
        open.add(node);
        generated++;
    }

    /** 离目标够近才值得动地形。 */
    private boolean near(BlockPos pos) {
        return Math.abs(pos.getX() - target.getX()) <= NEAR
                && Math.abs(pos.getZ() - target.getZ()) <= NEAR
                && Math.abs(pos.getY() - target.getY()) <= NEAR;
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
        final int blocks;
        final int steps;
        final int bridges;
        final int drops;
        final long seq;

        Node(BlockPos pos, Node parent, Kind kind, BlockPos block, boolean support,
                double g, double f, int blocks, int steps, int bridges, int drops, long seq) {
            this.pos = pos;
            this.parent = parent;
            this.kind = kind;
            this.block = block;
            this.support = support;
            this.g = g;
            this.f = f;
            this.blocks = blocks;
            this.steps = steps;
            this.bridges = bridges;
            this.drops = drops;
            this.seq = seq;
        }
    }
}
