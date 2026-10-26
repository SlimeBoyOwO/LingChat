package com.lingchat.maidbridge.action.mine;

/**
 * 挖掘的数目与算式，纯逻辑，不认识 Minecraft。
 *
 * <p>挖掘速度照原版的算法：正确工具是 {@code 速度 / 硬度 / 30}，用错了工具（或徒手挖
 * 需要工具才掉落的方块）是 {@code 速度 / 硬度 / 100}。车万女仆附属里的挖矿模组直接按
 * 30 算、不看工具对不对，那会让木镐挖黑曜石比原版快三倍多；玩家手上是什么工具、该挖多久，
 * 还是跟原版一致比较自然。
 */
public final class MineRules {
    /** 一次最多挖多少块：再多就该换个说法（比如分区挖），而不是让她在同一个指令里挖一整天。 */
    public static final int MAX_COUNT = 64;
    /** 她手到方块中心的距离。原版玩家的方块交互距离约 4.5 格，取同一个口径。 */
    public static final double REACH = 4.5;
    /** 找同类方块看多远。跟随模式下她本来就在主人附近活动，够用。 */
    public static final int FIND_RADIUS = 32;
    /** 玩家准星当区域锚点时看多远：指着远处的林子或矿壁也算数，不必凑到跟前。 */
    public static final double ANCHOR_RAY = 32.0;
    /** 指着的那一片多大：以准星落点为中心，这么远内的同类方块都算「那一片」。 */
    public static final int ANCHOR_RANGE = 8;
    /** 一次最多试几块：地形复杂时每一块都要找落脚点，全试一遍太贵。 */
    public static final int FIND_LIMIT = 12;
    /** 单块挖这么久还没成（含走路）就换个说法：卡在墙后、水底这类地方不留悬案。 */
    public static final int TARGET_TIMEOUT_TICKS = 1200;
    /** 为了够到目标最多先挖穿几块挡路的方块：再多就成了她自己在乱挖矿道。 */
    public static final int PATH_LIMIT = 24;
    /** 相对地址最远指多少格：再远就不像「她看见过的那一块」了。 */
    public static final int MAX_OFFSET = 48;
    /** 区域拆除一次最多拆多少块：与单次挖掘的上限一致，再多就分几次说。 */
    public static final int MAX_AREA_CELLS = 64;
    /** 长方体每条边最多跨几格：再大就不是「这一片」了。 */
    public static final int MAX_AREA_SPAN = 16;

    private MineRules() {
    }

    /** 相对地址：相对她脚下那一格的偏移，东、上、南。 */
    public record Offset(int dx, int dy, int dz) {
    }

    /** 长方体区域：两个相对的角，都是相对她脚下那一格的偏移。 */
    public record Area(Offset from, Offset to) {
        public boolean withinSpan() {
            return Math.abs(from.dx() - to.dx()) <= MAX_AREA_SPAN
                    && Math.abs(from.dy() - to.dy()) <= MAX_AREA_SPAN
                    && Math.abs(from.dz() - to.dz()) <= MAX_AREA_SPAN;
        }
    }

    /**
     * 解析相对地址：{@code @4,-2,-3} 或 {@code 4,-2,-3}，三个整数依次是东、上、南的偏移。
     *
     * <p>地址由「看一眼周围」的关系句给出，插件原样转回来。格式不对、分量超范围给 null，
     * 由调用方回一句「没看懂这个地址」，不猜一个近似的位置。
     */
    public static Offset parseOffset(String raw) {
        if (raw == null) {
            return null;
        }
        String text = raw.trim();
        // 模型常把整句「at=@4,-2,-3」抄回来，前缀可有可无
        if (text.regionMatches(true, 0, "at=", 0, 3)) {
            text = text.substring(3).trim();
        }
        if (text.startsWith("@")) {
            text = text.substring(1);
        }
        String[] parts = text.split(",", -1);
        if (parts.length != 3) {
            return null;
        }
        int[] values = new int[3];
        for (int i = 0; i < values.length; i++) {
            String part = parts[i].trim();
            if (part.isEmpty()) {
                return null;
            }
            try {
                values[i] = Integer.parseInt(part);
            } catch (NumberFormatException notANumber) {
                return null;
            }
            if (values[i] < -MAX_OFFSET || values[i] > MAX_OFFSET) {
                return null;
            }
        }
        return new Offset(values[0], values[1], values[2]);
    }

    /** 地址 → 协议里的写法，回执与模组内部的现场都走这一处。 */
    public static String formatOffset(int dx, int dy, int dz) {
        return "@" + dx + "," + dy + "," + dz;
    }

    /**
     * 解析区域：两个相对地址用空格或 {@code ~} 分开，如 {@code @-3,0,2 @3,3,4}。
     *
     * <p>两个角不分先后。格式不对或任意一条边超过 {@link #MAX_AREA_SPAN} 给 null，
     * 由调用方回一句「没看懂这个区域」，不悄悄缩成一个小一点的盒子。
     */
    public static Area parseArea(String raw) {
        if (raw == null) {
            return null;
        }
        String text = raw.trim();
        if (text.isEmpty()) {
            return null;
        }
        String[] parts = text.split("[\\s~]+");
        if (parts.length != 2) {
            return null;
        }
        Offset from = parseOffset(parts[0]);
        Offset to = parseOffset(parts[1]);
        if (from == null || to == null) {
            return null;
        }
        Area area = new Area(from, to);
        return area.withinSpan() ? area : null;
    }

    /** 请求里的块数：小于 1 按 1，超过上限按上限。 */
    public static int clampCount(int raw) {
        return Math.max(1, Math.min(MAX_COUNT, raw));
    }

    /** 硬度为负是「挖不动」（基岩那一类）。 */
    public static boolean mineable(float hardness) {
        return hardness >= 0.0F;
    }

    /**
     * 每 tick 的破坏进度。
     *
     * @param hardness    方块硬度，{@code getDestroySpeed}
     * @param toolSpeed   手上物品对这个方块的挖掘速度
     * @param correctTool 手上物品是不是挖这个方块的正确工具
     */
    public static float progressPerTick(float hardness, float toolSpeed, boolean correctTool) {
        if (!mineable(hardness)) {
            return 0.0F;
        }
        // 草、花这类硬度为 0 的方块一下就掉
        if (hardness == 0.0F) {
            return 1.0F;
        }
        float speed = Math.max(1.0F, toolSpeed);
        return correctTool ? speed / hardness / 30.0F : speed / hardness / 100.0F;
    }

    /** 破坏进度 → 客户端裂纹阶段（0~9）。原版按 {@code 进度 * 10 - 1} 取。 */
    public static int crackStage(float progress) {
        if (progress <= 0.0F) {
            return -1;
        }
        if (progress >= 1.0F) {
            return 9;
        }
        return Math.max(0, (int) (progress * 10.0F) - 1);
    }
}
