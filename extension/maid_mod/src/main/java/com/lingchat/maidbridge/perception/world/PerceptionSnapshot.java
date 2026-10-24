package com.lingchat.maidbridge.perception.world;


/**
 * 一次感知采样的结果。
 *
 * <p>特意做成不认识 Minecraft 的纯数据：读游戏填它的是 {@link PerceptionReader}，
 * 判定（{@link PerceptionDiff}）也只认它，于是那部分逻辑能脱开游戏跑单测。
 *
 * @param entityId 被监控的女仆 uuid，换了监控对象就该重出一份快照
 * @param dimension 维度 id，如 {@code minecraft:overworld}
 * @param biome 生物群系 id，如 {@code minecraft:plains}；取不到时为空串
 * @param hasSky 这个维度有没有天空。下界 / 末地没有，那里的「天气」「昼夜」没有意义
 * @param raining 在下雨（没有天空的维度恒为 false）
 * @param thundering 在打雷
 * @param dayTime 世界时间（tick），用来判昼夜
 * @param playerName 玩家名；女仆没有主人时为空串
 * @param playerHealth 玩家血量；没有主人时为 0
 * @param playerMaxHealth 玩家血量上限；没有主人时为 0
 * @param maidName 女仆名
 * @param maidHealth 女仆血量
 * @param maidMaxHealth 女仆血量上限
 * @param posture 她此刻的姿势：{@code standing} / {@code sitting} / {@code sleeping} / {@code lying}
 *                / {@code unconscious}
 */
public record PerceptionSnapshot(
        String entityId,
        String dimension,
        String biome,
        boolean hasSky,
        boolean raining,
        boolean thundering,
        long dayTime,
        String playerName,
        double playerHealth,
        double playerMaxHealth,
        String maidName,
        double maidHealth,
        double maidMaxHealth,
        String posture) {
}
