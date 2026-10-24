package com.lingchat.maidbridge.perception.query;

/**
 * 相连成团的一种方块。
 *
 * <p>位置取团里离扫描中心最近的那一块，与 {@link BlockRelation} 同一套口径。
 * {@code cells} 是这一团数到的格数，数到上限就停，{@code truncated} 说明实际更多。
 */
public record BlockCluster(
        String id,
        int cells,
        boolean truncated,
        int dx,
        int dy,
        int dz,
        String facing,
        double distance,
        int blockers,
        String blocker) {
}
