package com.lingchat.maidbridge.action.core;

import java.util.List;

/**
 * 从候选中挑出最近的、能睡的床。
 *
 * <p>纯逻辑：游戏侧只把 POI 索引里查到的东西拍成 {@link Candidate} 送进来，这里不碰世界。
 * 判据一律用方块事实（是床、是床头、没被占）而不是 POI 的登记状态——床可能已经被挖掉
 * 或换成别的方块，索引会过期。
 */
public final class BedChoice {
    /** 原版床的 POI：床头的 {@code minecraft:home}。 */
    private static final String VANILLA_POI = "minecraft:home";
    /** 车万女仆自己的女仆床（它不是原版 BedBlock，走自己的 POI）。 */
    private static final String MAID_BED_POI = "touhou_little_maid:maid_bed";

    private BedChoice() {
    }

    /**
     * 一个候选方块。
     *
     * @param bedBlock 这里确实还是床（索引没过期）
     * @param head 是床头那一格（原版与车万女仆的床都只把床头登记成 POI）
     * @param occupied 已经被占了
     */
    public record Candidate(int x, int y, int z, boolean bedBlock, boolean head, boolean occupied) {
    }

    /** 挑出来的结果：`pos` 为 null 表示没得睡，这时看 `sawOccupied` 区分「附近没床」与「床都被占了」。 */
    public record Pick(Candidate pos, boolean sawOccupied) {
    }

    /** 这个 POI 算不算床。索引里还有别的兴趣点（工作方块、稻草人之类），不能全当床。 */
    public static boolean isBedPoiId(String poiId) {
        return VANILLA_POI.equals(poiId) || MAID_BED_POI.equals(poiId);
    }

    /** 最近的一张空床；等距时按坐标取定一个，保证同一个世界里每次挑出来的是同一张。 */
    public static Pick pick(List<Candidate> candidates, int fromX, int fromY, int fromZ) {
        Candidate best = null;
        long bestDistance = Long.MAX_VALUE;
        boolean sawOccupied = false;
        for (Candidate candidate : candidates) {
            if (!candidate.bedBlock() || !candidate.head()) {
                continue;
            }
            if (candidate.occupied()) {
                sawOccupied = true;
                continue;
            }
            long distance = distanceSqr(candidate, fromX, fromY, fromZ);
            if (distance < bestDistance || (distance == bestDistance && isTieBreakWinner(candidate, best))) {
                best = candidate;
                bestDistance = distance;
            }
        }
        return new Pick(best, sawOccupied);
    }

    private static long distanceSqr(Candidate candidate, int x, int y, int z) {
        long dx = candidate.x() - x;
        long dy = candidate.y() - y;
        long dz = candidate.z() - z;
        return dx * dx + dy * dy + dz * dz;
    }

    /** 等距时按 x → y → z 排序取一个固定的；否则结果取决于遍历顺序，行为不可复现。 */
    private static boolean isTieBreakWinner(Candidate candidate, Candidate current) {
        if (current == null) {
            return true;
        }
        if (candidate.x() != current.x()) {
            return candidate.x() < current.x();
        }
        if (candidate.y() != current.y()) {
            return candidate.y() < current.y();
        }
        return candidate.z() < current.z();
    }
}
