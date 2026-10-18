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
    public static final int FIND_RADIUS = 24;
    /** 玩家准星当区域锚点时看多远：指着远处的林子或矿壁也算数，不必凑到跟前。 */
    public static final double ANCHOR_RAY = 32.0;
    /** 指着的那一片多大：以准星落点为中心，这么远内的同类方块都算「那一片」。 */
    public static final int ANCHOR_RANGE = 8;
    /** 一次最多试几块：地形复杂时每一块都要找落脚点，全试一遍太贵。 */
    public static final int FIND_LIMIT = 8;
    /** 单块挖这么久还没成（含走路）就换个说法：卡在墙后、水底这类地方不留悬案。 */
    public static final int TARGET_TIMEOUT_TICKS = 1200;

    private MineRules() {
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
