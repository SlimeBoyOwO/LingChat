package com.lingchat.maidbridge.perception.query;

/**
 * 一种方块相对她的位置关系。
 *
 * <p>坐标一律是相对她脚下那一格的整数偏移，方位相对身体朝向；插件据此说
 * 「正前方三步、同一层」，不把世界坐标交给模型。
 *
 * <p>{@code walkSteps} 只在八格内、走得通时给正数，判不出来是 -1。
 * {@code blockers} 是视线中间隔着的实心方块数，超上限就按上限算。
 */
public record BlockRelation(
        String id,
        boolean seenNow,
        int dx,
        int dy,
        int dz,
        String facing,
        boolean sameLayer,
        double distance,
        int blockers,
        String blocker,
        int walkSteps) {
}
