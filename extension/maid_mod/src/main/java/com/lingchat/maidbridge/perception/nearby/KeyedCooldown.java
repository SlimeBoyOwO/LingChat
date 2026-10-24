package com.lingchat.maidbridge.perception.nearby;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * 按 key 记「上次是什么时候报的」，用来给「一群同类」挨个定冷静期。
 *
 * <p>现在的用处是身边来了哪种活物：牧场里一只羊不该每半分钟被提一次，所以按生物 id 各记
 * 一个时间。表有上限，超了就挤掉最久没提过的那个——不然在牧场待久了这张表会一直长。
 */
public final class KeyedCooldown {
    /** 记着这么多种为止。 */
    private static final int LIMIT = 16;

    /** key → 上次报的 tick。放之前先删，于是最前面那个总是「最久没提过」的。 */
    private final Map<String, Long> lastTick = new LinkedHashMap<>();

    /** 现在允许为这个 key 报一次吗；允许就把时间记上（调用方不必再记一次）。 */
    public boolean allow(String key, long nowTick, long cooldownTicks) {
        Long last = lastTick.get(key);
        if (last != null && nowTick < last + cooldownTicks) {
            return false;
        }
        lastTick.remove(key);
        lastTick.put(key, nowTick);
        while (lastTick.size() > LIMIT) {
            lastTick.remove(lastTick.keySet().iterator().next());
        }
        return true;
    }

    /** 换了监控对象或重新接上时清掉：上一只女仆身边有谁，与眼下无关。 */
    public void reset() {
        lastTick.clear();
    }
}
