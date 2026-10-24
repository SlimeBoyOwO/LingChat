package com.lingchat.maidbridge.perception.spatial;

import net.minecraft.core.BlockPos;

/**
 * 方块清单里的一种方块、最近那一格，以及它是此刻看见的，还是刚才看见、现在不在视野里的。
 *
 * <p>插件据此分开说「眼前这一片」与「刚才在附近见过」；后者有出处，不是全知。
 * 位置只给投影算方位与步数，不直接进协议。
 */
public record BlockSight(String id, boolean seenNow, BlockPos pos) {
}
