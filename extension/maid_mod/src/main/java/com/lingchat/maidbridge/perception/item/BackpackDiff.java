package com.lingchat.maidbridge.perception.item;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;

/**
 * 背包差分的纯逻辑：这一帧比上一帧多了什么。
 *
 * <p>不是为了盯着她的家当，而是想知道玩家什么时候把东西给了她。她自带的交接口是在女仆
 * 界面里把物品放进背包，那条路没有任何事件可挂，只能前后比一次。
 *
 * <p>捡取另有精确事件（{@code MaidPickupEvent}），它同样会改背包；不扣掉就会被说两遍，
 * 所以这里收一份「已经由捡取事件解释过」的账。
 */
public final class BackpackDiff {
    /** 多出来的一件东西。 */
    public record Gain(String item, int count) {
    }

    private BackpackDiff() {
    }

    /**
     * {@code after} 比 {@code before} 多出来、且没有被捡取事件解释过的那部分。
     *
     * <p>只看增加：拿出去、换位置、合并堆叠都不出声。
     */
    public static List<Gain> gains(Map<String, Integer> before, Map<String, Integer> after,
                                   Map<String, Integer> explained) {
        List<Gain> gains = new ArrayList<>();
        for (String item : new TreeSet<>(after.keySet())) {
            int delta = after.getOrDefault(item, 0) - before.getOrDefault(item, 0);
            if (delta <= 0) {
                continue;
            }
            int rest = delta - explained.getOrDefault(item, 0);
            if (rest > 0) {
                gains.add(new Gain(item, rest));
            }
        }
        return gains;
    }
}
