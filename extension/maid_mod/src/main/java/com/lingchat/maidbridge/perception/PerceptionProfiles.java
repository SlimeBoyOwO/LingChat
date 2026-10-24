package com.lingchat.maidbridge.perception;

import com.github.tartaricacid.touhoulittlemaid.config.subconfig.MaidConfig;
import com.lingchat.maidbridge.MaidBridgeConfig;
import com.lingchat.maidbridge.perception.nearby.Surroundings;

/**
 * 感知分层的半径与上限都收在这一处。
 *
 * <p>两层：
 *
 * <ul>
 *   <li>自动感知（周期采样；插件在出事那条旁白里顺手补一份也用它）：看得见的目标按车万女仆的工作范围
 *       （MaidWorkRange，默认 12）算，看不见的只算听到半径（默认 6）以内的声音。</li>
 *   <li>按需查询（玩家明确问「你周围有什么」）：看得见的目标放宽到 32 格，好答得出
 *       十几格外的目标；看不见的仍只算听到半径。</li>
 * </ul>
 *
 * <p>两档都最多列 3 个（{@link Surroundings#NEARBY_LIMIT}）：感知是给对话用的，
 * 不是雷达。
 */
public final class PerceptionProfiles {
    /** 按需查询看得见的半径（格）。 */
    public static final double QUERY_VISIBLE_RADIUS = 32.0;
    /** 工作范围再大也不会把感知拉到这之外。 */
    private static final int MAX_VISIBLE_RADIUS = 32;

    private PerceptionProfiles() {
    }

    /** 她自发的注意：看得见看工作范围，看不见只听很近的声音。 */
    public static Surroundings.Profile auto() {
        double work = Math.min(MaidConfig.MAID_WORK_RANGE.get(), MAX_VISIBLE_RADIUS);
        return Surroundings.Profile.of(
                Math.max(MaidBridgeConfig.NOTICE_RADIUS.get(), work),
                MaidBridgeConfig.HEARD_RADIUS.get());
    }

    /** 玩家问「你周围有什么」：看得更远，但仍然只列最近几个。 */
    public static Surroundings.Profile query() {
        return Surroundings.Profile.of(QUERY_VISIBLE_RADIUS, MaidBridgeConfig.HEARD_RADIUS.get());
    }
}
