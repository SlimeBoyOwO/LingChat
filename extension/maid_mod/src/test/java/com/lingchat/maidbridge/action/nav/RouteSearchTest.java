package com.lingchat.maidbridge.action.nav;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

import com.lingchat.maidbridge.action.nav.RouteSearch.Kind;
import com.lingchat.maidbridge.action.nav.RouteSearch.Step;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import net.minecraft.core.BlockPos;
import org.junit.jupiter.api.Test;

/** 落脚点图搜索：走、跑跳、搭路、垫高各走一遍，确认路线是搜出来的而不是排出来的。 */
class RouteSearchTest {
    private static final int FLOOR = 0;
    private static final int FEET = 1;

    /** 一小块测试地形：y=0 摆地板，y=1 站人；跑跳与直落要单独许可。 */
    private static final class Fake implements RouteSearch.View {
        final Set<BlockPos> blocks = new HashSet<>();
        final Set<BlockPos> goals = new HashSet<>();
        final Set<String> hops = new HashSet<>();
        final Set<String> drops = new HashSet<>();
        /** 默认允许随便垫脚；验盒子边界时关掉，免得虚拟楼梯把节点额度吃光。 */
        boolean place = true;

        Fake line(int x0, int x1) {
            for (int x = x0; x <= x1; x++) {
                blocks.add(new BlockPos(x, FLOOR, 0));
            }
            return this;
        }

        Fake goal(int x, int y) {
            goals.add(new BlockPos(x, y, 0));
            return this;
        }

        @Override
        public boolean standable(BlockPos pos) {
            return solid(pos.below()) && air(pos) && air(pos.above());
        }

        @Override
        public boolean air(BlockPos pos) {
            return !solid(pos);
        }

        @Override
        public boolean solid(BlockPos pos) {
            return blocks.contains(pos);
        }

        @Override
        public boolean passable(BlockPos pos) {
            return standable(pos);
        }

        @Override
        public boolean placeable(BlockPos block, BlockPos from) {
            return place && air(block);
        }

        @Override
        public boolean fits(BlockPos stand, BlockPos target) {
            return true;
        }

        @Override
        public boolean hop(BlockPos from, BlockPos to, boolean builtFloor) {
            return hops.contains(key(from, to));
        }

        @Override
        public boolean drop(BlockPos from, BlockPos to, boolean builtFloor) {
            return drops.contains(key(from, to));
        }

        @Override
        public boolean breakable(BlockPos pos) {
            return solid(pos);
        }

        @Override
        public boolean goal(BlockPos pos) {
            return goals.contains(pos);
        }
    }

    private static String key(BlockPos from, BlockPos to) {
        return from.toShortString() + ">" + to.toShortString();
    }

    private static RouteSearch.Limits limits(int bridges) {
        return new RouteSearch.Limits(bridges);
    }

    /** 常规用例的盒子开得足够大，只验路线本身；盒子边界由专门的用例验。 */
    private static final int PAD = 16;

    private static List<Step> solve(Fake world, BlockPos target) {
        return solve(world, target, limits(8), PAD);
    }

    private static List<Step> solve(Fake world, BlockPos target, RouteSearch.Limits limits) {
        return solve(world, target, limits, PAD);
    }

    private static List<Step> solve(
            Fake world, BlockPos target, RouteSearch.Limits limits, int pad) {
        return solve(world, target, limits, pad, true, true);
    }

    private static List<Step> solve(Fake world, BlockPos target, RouteSearch.Limits limits,
            int pad, boolean bridges, boolean breaks) {
        RouteSearch search = RouteSearch.start(
                world, new BlockPos(0, FEET, 0), target, limits, pad, bridges, breaks);
        for (int i = 0; i < 200; i++) {
            RouteSearch.State state = search.step();
            if (state == RouteSearch.State.FOUND) {
                return search.route();
            }
            if (state == RouteSearch.State.EXHAUSTED) {
                return List.of();
            }
        }
        fail("搜索没有收敛");
        return List.of();
    }

    @Test
    void walksAcrossOpenGround() {
        Fake world = new Fake().line(0, 6).goal(4, FEET);
        List<Step> route = solve(world, new BlockPos(6, FEET, 0));
        // 连续几格走合成一次寻路，别一格一次
        assertEquals(1, route.size());
        assertEquals(Kind.WALK, route.get(0).kind());
        assertEquals(new BlockPos(4, FEET, 0), route.get(0).to());
    }

    @Test
    void hopsAcrossAGap() {
        Fake world = new Fake().line(0, 1).line(5, 7).goal(5, FEET);
        world.hops.add(key(new BlockPos(1, FEET, 0), new BlockPos(5, FEET, 0)));
        List<Step> route = solve(world, new BlockPos(7, FEET, 0));
        assertEquals(2, route.size());
        assertEquals(Kind.WALK, route.get(0).kind());
        assertEquals(Kind.HOP, route.get(1).kind());
        assertEquals(new BlockPos(5, FEET, 0), route.get(1).to());
    }

    @Test
    void bridgesWhenJumpingIsNotAllowed() {
        Fake world = new Fake().line(0, 1).line(5, 7).goal(5, FEET);
        List<Step> route = solve(world, new BlockPos(7, FEET, 0));
        long bridges = route.stream().filter(step -> step.kind() == Kind.BRIDGE).count();
        assertEquals(3, bridges);
        assertEquals(new BlockPos(5, FEET, 0), route.get(route.size() - 1).to());
    }

    @Test
    void bridgesAGapEvenWhenTheTargetIsFarAway() {
        // 缺口离目标三十多格：动地形不该只在目标附近才算数，远处的缺口一样是路
        Fake world = new Fake().line(0, 1).line(5, 40).goal(38, FEET);
        List<Step> route = solve(world, new BlockPos(40, FEET, 0));
        long bridges = route.stream().filter(step -> step.kind() == Kind.BRIDGE).count();
        assertEquals(3, bridges);
        assertEquals(new BlockPos(38, FEET, 0), route.get(route.size() - 1).to());
    }

    @Test
    void risesToAStandThatDoesNotExistYet() {
        Fake world = new Fake().line(0, 0).goal(1, 2);
        List<Step> route = solve(world, new BlockPos(2, 2, 0));
        // 目标站位现在还是空气：靠这一格垫出来的方块才算站得住
        assertEquals(1, route.size());
        assertEquals(Kind.RISE, route.get(0).kind());
        assertTrue(route.get(0).support());
        assertEquals(new BlockPos(1, 2, 0), route.get(0).to());
    }

    @Test
    void onlySearchesInsideTheBoxUntilItIsWidened() {
        // 直线上立一堵两格高的墙，唯一的路是从 z=-6 绕过去
        Fake world = new Fake().line(0, 20).goal(20, FEET);
        world.place = false;
        world.blocks.add(new BlockPos(10, FEET, 0));
        world.blocks.add(new BlockPos(10, FEET + 1, 0));
        for (int z = -6; z <= 0; z++) {
            world.blocks.add(new BlockPos(9, FLOOR, z));
            world.blocks.add(new BlockPos(11, FLOOR, z));
        }
        for (int x = 9; x <= 11; x++) {
            world.blocks.add(new BlockPos(x, FLOOR, -6));
        }
        BlockPos target = new BlockPos(20, FEET, 0);
        // 盒子只扩三格时绕不过去；扩到七格就找得到（拆墙这一档先关掉）
        assertTrue(solve(world, target, limits(8), 3, false, false).isEmpty());
        assertTrue(!solve(world, target, limits(8), 7, false, false).isEmpty());
    }

    @Test
    void breaksThroughAWallOnlyWhenBreakingIsAllowed() {
        // 中间一堵两格高的实心墙，墙后就是目标；不许垫脚、也没有跑跳
        Fake world = new Fake().line(0, 10).goal(8, FEET);
        world.place = false;
        world.blocks.add(new BlockPos(5, FEET, 0));
        world.blocks.add(new BlockPos(5, FEET + 1, 0));
        BlockPos target = new BlockPos(8, FEET, 0);
        // 不许拆时绕不过去
        assertTrue(solve(world, target, limits(8), PAD, true, false).isEmpty());
        // 允许拆就穿墙过去：先走到墙前，拆墙，再接着走
        List<Step> route = solve(world, target, limits(8), PAD, true, true);
        assertEquals(Kind.WALK, route.get(0).kind());
        assertEquals(Kind.BREAK, route.get(1).kind());
        assertEquals(new BlockPos(5, FEET, 0), route.get(1).to());
    }

    @Test
    void doesNotBridgeWhenTheTierForbidsIt() {
        Fake world = new Fake().line(0, 1).line(5, 7).goal(5, FEET);
        // 跑跳没许可、这一档又不许搭路：缺口跨不过去
        assertTrue(solve(world, new BlockPos(7, FEET, 0), limits(8), PAD, false, false).isEmpty());
    }

    @Test
    void givesUpWhenThereIsNothingToBuildWith() {
        Fake world = new Fake().line(0, 1).line(3, 5).goal(3, FEET);
        // 不许跑跳、身上也没有垫脚料：缺口跨不过去
        world.place = false;
        assertEquals(List.of(), solve(world, new BlockPos(5, FEET, 0), limits(0)));
    }

    /** 两段平台中间一条两格深的沟：走得下去，爬不上来。 */
    private static Fake trench() {
        Fake world = new Fake().line(0, 1).line(5, 7).goal(5, FEET);
        for (int x = 2; x <= 4; x++) {
            world.blocks.add(new BlockPos(x, FLOOR - 2, 0));
        }
        return world;
    }

    @Test
    void hopsOverAShallowTrench() {
        // 沟底走得下去，不能因此就不考虑跳过去
        Fake world = trench();
        world.place = false;
        world.hops.add(key(new BlockPos(1, FEET, 0), new BlockPos(5, FEET, 0)));
        List<Step> route = solve(world, new BlockPos(7, FEET, 0), limits(8), PAD, false, false);
        assertTrue(route.stream().anyMatch(step -> step.kind() == Kind.HOP));
        assertEquals(new BlockPos(5, FEET, 0), route.get(route.size() - 1).to());
    }

    @Test
    void bridgesLowOverGround() {
        // 一格宽的沟，沟底三格下就是地面：平铺一格比跳下去再垫上来省
        Fake world = new Fake().line(0, 1).line(3, 5).goal(3, FEET);
        world.blocks.add(new BlockPos(2, FLOOR - 3, 0));
        List<Step> route = solve(world, new BlockPos(5, FEET, 0), limits(8), PAD, true, false);
        long bridges = route.stream().filter(step -> step.kind() == Kind.BRIDGE).count();
        assertEquals(1, bridges);
        assertEquals(new BlockPos(3, FEET, 0), route.get(route.size() - 1).to());
    }

    @Test
    void pillarsSeveralBlocksStraightUp() {
        // 旁边悬着一块平台，人站在 x=1 那格顶上；原地连垫两格再跨上去
        Fake world = new Fake().line(0, 0).goal(1, 4);
        world.blocks.add(new BlockPos(1, 3, 0));
        List<Step> route = solve(world, new BlockPos(1, 4, 0));
        assertEquals(List.of(Kind.PILLAR, Kind.PILLAR, Kind.WALK),
                route.stream().map(Step::kind).toList());
        assertEquals(new BlockPos(1, 4, 0), route.get(route.size() - 1).to());
    }

    @Test
    void hopsDiagonally() {
        // 落点在斜前方错开两格：只能斜着跳过去
        Fake world = new Fake().line(0, 0);
        world.place = false;
        BlockPos landing = new BlockPos(3, FEET, 2);
        world.blocks.add(landing.below());
        world.goals.add(landing);
        world.hops.add(key(new BlockPos(0, FEET, 0), landing));
        List<Step> route = solve(world, landing);
        assertEquals(1, route.size());
        assertEquals(Kind.HOP, route.get(0).kind());
        assertEquals(landing, route.get(0).to());
    }
}
