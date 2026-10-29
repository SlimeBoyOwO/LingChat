package com.lingchat.maidbridge.perception.nearby;

import java.util.List;
import java.util.Map;

/**
 * 脚下这块方块算不算「值得说一声的地」。
 *
 * <p>白名单只有这么几种：走在石头、泥土、木板上不该出声，否则她在自己家里转一圈就能刷出
 * 十条旁白。读世界那一侧（{@link SurroundingsReader}）用它填快照里的 {@code footing}，
 * 判定那一侧（{@link SurroundingsDiff}）用它判「脚下换没换种地」——两边共用同一张表，
 * 协议里的 token 与判定口径因此不会走岔。
 */
public final class GroundKind {
    /** 方块 id → 类别；没收录的返回空串，一律不报。液体不在这里：泡进什么液体另有事件。 */
    private static final Map<String, String> BY_BLOCK = Map.ofEntries(
            Map.entry("minecraft:fire", "fire"),
            Map.entry("minecraft:soul_fire", "fire"),
            Map.entry("minecraft:powder_snow", "snow"),
            Map.entry("minecraft:snow_block", "snow"),
            Map.entry("minecraft:snow", "snow"),
            Map.entry("minecraft:sand", "sand"),
            Map.entry("minecraft:red_sand", "sand"),
            Map.entry("minecraft:farmland", "farmland"),
            Map.entry("minecraft:ice", "ice"),
            Map.entry("minecraft:packed_ice", "ice"),
            Map.entry("minecraft:blue_ice", "ice"));

    /** 踩到火上要立刻说，不受冷静期挡。 */
    private static final List<String> URGENT = List.of("fire");

    private GroundKind() {
    }

    /** 脚下方块属于哪一类；不值得说的返回空串。 */
    public static String of(String blockId) {
        return BY_BLOCK.getOrDefault(blockId == null ? "" : blockId, "");
    }

    public static boolean isUrgent(String kind) {
        return URGENT.contains(kind);
    }
}
