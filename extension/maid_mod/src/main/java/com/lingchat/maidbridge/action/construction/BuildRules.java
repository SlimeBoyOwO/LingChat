package com.lingchat.maidbridge.action.construction;

/**
 * 施工的纯规则：一格的生命周期与一层走完后的去向。
 *
 * <p>不碰世界、不碰女仆，输入全是已经数好的状态，方便单测。控制器只负责收集这些
 * 状态并执行返回的动作。
 */
final class BuildRules {
    /** 一层连续几遍没有新进展就判这一层剩格不可达，换下一层。 */
    static final int MAX_STALL_PASSES = 3;

    private BuildRules() {
    }

    /** 一格在整轮施工里的进度。补试只有一轮，所以这是一条无环的状态链。 */
    enum CellState {
        /** 还没轮到它。 */
        PENDING,
        /** 这一遍没做成，等整张图纸过完补试。 */
        DEFERRED,
        /** 正在补试那一轮里。 */
        RETRYING,
        /** 图纸上的方块已经放好。 */
        DONE,
        /** 有定论，收尾时计入失败。 */
        FAILED;

        /** 还值得为它努力：扫描和选格只看这两种。 */
        boolean open() {
            return this == PENDING || this == RETRYING;
        }
    }

    /** 一格没放上的原因；code 是推给插件的线格式，改动要同步事件文案。 */
    enum Failure {
        NO_STAND("no_stand"),
        NO_SIGHT("no_sight"),
        NO_HEADROOM("no_headroom"),
        OCCUPIED("blocked"),
        BLOCKED_BY_ENTITY("blocked_by_entity"),
        SELF_BLOCKED("self_blocked"),
        LIQUID("liquid"),
        OUT_OF_REACH("out_of_reach"),
        PLACE_FAILED("place_failed"),
        NO_PATH("no_path"),
        SHORT_OF_MATERIAL("short_of_material");

        final String code;

        Failure(String code) {
            this.code = code;
        }
    }

    /** 一层收尾时要做的事。第二动作在第一动作（搁置或收账）之后执行。 */
    enum Action {
        NONE,
        RESCAN,
        NEXT_LAYER,
        RETRY_ROUND,
        DEFER_LAYER,
        SETTLE_LAYER,
        FINISH
    }

    /** 一格此刻的本地情况：只看方块、库存、活物，不做站位搜索和视线判定。 */
    record Situation(
            boolean matches,
            boolean liquid,
            boolean occupied,
            boolean canDestroy,
            boolean hasMaterial,
            boolean entityHere,
            boolean selfHere) {
    }

    /** 本地情况对应的结论；CONTINUE 表示世界这边没问题，再去看距离与视线。 */
    enum Verdict {
        DONE,
        LIQUID,
        /** 被别的方块占着，挖得动：够得着就地挖，够不着走过去挖。 */
        DIG,
        /** 被别的方块占着，不归她挖。 */
        DIG_BLOCKED,
        MISSING,
        HELD_BY_ENTITY,
        SELF_INSIDE,
        CONTINUE
    }

    /** 判定顺序就是优先级，从上到下命中即止。 */
    static Verdict verdict(Situation situation) {
        if (situation.matches()) {
            return Verdict.DONE;
        }
        if (situation.liquid()) {
            return Verdict.LIQUID;
        }
        if (situation.occupied()) {
            return situation.canDestroy() ? Verdict.DIG : Verdict.DIG_BLOCKED;
        }
        if (!situation.hasMaterial()) {
            return Verdict.MISSING;
        }
        if (situation.entityHere()) {
            return Verdict.HELD_BY_ENTITY;
        }
        if (situation.selfHere()) {
            return Verdict.SELF_INSIDE;
        }
        return Verdict.CONTINUE;
    }

    record PassEnd(Action first, Action second) {
        static final PassEnd FINISH = new PassEnd(Action.FINISH, Action.NONE);
        static final PassEnd RESCAN = new PassEnd(Action.RESCAN, Action.NONE);
    }

    /**
     * 一层收尾时的全部输入。
     *
     * @param allPlaced 整张图纸都放好了
     * @param layerOpen 本层还有开着的格子（只剩门时按补试口径算门）
     * @param openHeld 本层剩下的开格全在等活物挪开
     * @param progress 这一遍有进展：放了方块或垫了脚
     * @param stallPasses 这一遍之后连续没进展的遍数
     * @param retried 已经补试过一轮
     * @param canMoveOn 别的层还有开格，或只剩门可以装门
     * @param canRetryRound 还有搁置的格子可以补试
     */
    record PassStats(
            boolean allPlaced,
            boolean layerOpen,
            boolean openHeld,
            boolean progress,
            int stallPasses,
            boolean retried,
            boolean canMoveOn,
            boolean canRetryRound) {
    }

    /** 一层扫完之后去哪：再扫一遍、换层、补试、搁置收账，或收工。 */
    static PassEnd endPass(PassStats stats) {
        if (stats.allPlaced()) {
            return PassEnd.FINISH;
        }
        if (!stats.layerOpen()) {
            if (stats.canMoveOn()) {
                return new PassEnd(Action.NEXT_LAYER, Action.NONE);
            }
            if (stats.canRetryRound()) {
                return new PassEnd(Action.RETRY_ROUND, Action.NONE);
            }
            return PassEnd.FINISH;
        }
        if (stats.openHeld() || stats.progress()) {
            return PassEnd.RESCAN;
        }
        if (stats.stallPasses() < MAX_STALL_PASSES) {
            return PassEnd.RESCAN;
        }
        // 这一遍到头了：补试过的直接收账，没补试过的先搁着，等整张图纸过完再回来
        Action first = stats.retried() ? Action.SETTLE_LAYER : Action.DEFER_LAYER;
        // 走到这里说明本层还有开格：没补试过的搁置之后就有格子可以补试了，
        // 哪怕此刻一枚 deferred 都还没有
        boolean retryAfter = stats.canRetryRound() || !stats.retried();
        if (stats.canMoveOn()) {
            return new PassEnd(first, Action.NEXT_LAYER);
        }
        if (retryAfter) {
            return new PassEnd(first, Action.RETRY_ROUND);
        }
        return new PassEnd(first, Action.FINISH);
    }
}
