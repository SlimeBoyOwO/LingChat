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
            return air(block);
        }

        @Override
        public boolean fits(BlockPos stand, BlockPos target) {
            return true;
        }

        @Override
        public boolean hop(BlockPos from, BlockPos to) {
            return hops.contains(key(from, to));
        }

        @Override
        public boolean drop(BlockPos from, BlockPos to) {
            return drops.contains(key(from, to));
        }

        @Override
        public boolean goal(BlockPos pos) {
            return goals.contains(pos);
        }
    }

    private static String key(BlockPos from, BlockPos to) {
        return from.toShortString() + ">" + to.toShortString();
    }

    private static RouteSearch.Limits limits(int blocks, int steps, int bridges, int drops) {
        return new RouteSearch.Limits(blocks, steps, bridges, drops);
    }

    private static List<Step> solve(Fake world, BlockPos target) {
        return solve(world, target, limits(16, 12, 8, 2));
    }

    private static List<Step> solve(Fake world, BlockPos target, RouteSearch.Limits limits) {
        RouteSearch search = RouteSearch.start(
                world, new BlockPos(0, FEET, 0), target, limits);
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
    void givesUpWhenThereIsNoWayAcross() {
        Fake world = new Fake().line(0, 1).line(3, 5).goal(3, FEET);
        // 不许跑跳，也没有垫脚额度：缺口跨不过去
        assertEquals(List.of(), solve(world, new BlockPos(5, FEET, 0), limits(0, 0, 0, 0)));
    }
}
