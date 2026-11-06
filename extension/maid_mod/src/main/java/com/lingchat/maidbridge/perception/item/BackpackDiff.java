package com.lingchat.maidbridge.perception.item;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;

/**
 * 背包差分的纯逻辑：这一帧比上一帧多了什么、少了什么。
 *
 * <p>不是为了盯着她的家当，而是想知道玩家什么时候把东西给了她、什么时候从她背包里拿走。
 * 女仆界面里的交接没有任何事件可挂，只能前后比一次。
 *
 * <p>捡取另有精确事件（{@code MaidPickupEvent}），它同样会改背包；不扣掉就会被说两遍，
 * 所以这里收一份「已经由别处解释过」的账：多出来那份扣拾取，少掉那份扣她自己吃掉的、
 * 交给主人的。
 */
public final class BackpackDiff {
    /** 多出来的一件东西。 */
    public record Gain(String item, int count) {
    }

    /** 少掉的一件东西。 */
    public record Loss(String item, int count) {
    }

    private BackpackDiff() {
    }

    /**
     * {@code after} 比 {@code before} 多出来、且没有被捡取事件解释过的那部分。
     *
     * <p>只看增加：换位置、合并堆叠都不出声。
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

    /**
     * {@code after} 比 {@code before} 少掉、又不是她自己弄没的那部分。
     *
     * <p>玩家在界面里从她背包拿东西也没有事件可挂，同样只能前后比一次。她自己吃掉的、
     * 交给主人的那些由调用方预先记进 {@code explained}。
     */
    public static List<Loss> losses(Map<String, Integer> before, Map<String, Integer> after,
                                    Map<String, Integer> explained) {
        List<Loss> losses = new ArrayList<>();
        for (String item : new TreeSet<>(before.keySet())) {
            int delta = before.getOrDefault(item, 0) - after.getOrDefault(item, 0);
            if (delta <= 0) {
                continue;
            }
            int rest = delta - explained.getOrDefault(item, 0);
            if (rest > 0) {
                losses.add(new Loss(item, rest));
            }
        }
        return losses;
    }
}
