package com.lingchat.maidbridge.perception.spatial;

import java.util.List;

/**
 * 她一帧里看见的危险方块：哪一种、大概多远、在哪个方向。
 *
 * <p>只放 {@link SpatialReader} 读出来、{@link SpatialDiff} 要比的那几项。位置不进协议：
 * 插件只用种类与远近措辞，方位留着给以后按需查询用。
 */
public record SpatialView(String entityId, List<Hazard> hazards) {
    /** 某一种危险离她最近的那一格。 */
    public record Hazard(String id, double distance, String direction) {
    }
}
