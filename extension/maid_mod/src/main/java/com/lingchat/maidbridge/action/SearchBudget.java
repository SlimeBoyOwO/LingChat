package com.lingchat.maidbridge.action;

/**
 * 一次 tick 里还愿意花几次寻路：房子盖到一半、屋里屋外不通时，逐格逐候选地算路会把
 * 服务端拖住（每个候选都要在整片地形上搜一遍）。额度用完就先歇，下一 tick 接着算。
 */
public final class SearchBudget {
    private int left;

    public SearchBudget(int left) {
        this.left = Math.max(0, left);
    }

    /** 领一次寻路额度；没额度了给假。 */
    public boolean take() {
        if (left <= 0) {
            return false;
        }
        left--;
        return true;
    }

    /** 这一 tick 的额度已经花完。 */
    public boolean spent() {
        return left <= 0;
    }
}
