package com.lingchat.maidbridge.perception.query;

import java.util.List;

/**
 * 一次按需查询的投影结果：关系、相连团，以及玩家问路时才生成的小图。
 *
 * <p>方块清单本身仍由 {@code Surroundings.response} 带出去，这里只放新增的几种看法。
 */
public record BlockProjection(
        List<BlockRelation> relations,
        List<BlockCluster> clusters,
        MapProjection map) {
}
