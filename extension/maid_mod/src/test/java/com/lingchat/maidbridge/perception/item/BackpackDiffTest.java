package com.lingchat.maidbridge.perception.item;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;

/** 背包差分：只报多出来的，捡取已经报过的扣掉，拿走与挪位置都不出声。 */
class BackpackDiffTest {

    private static Map<String, Integer> counts(Object... pairs) {
        Map<String, Integer> map = new LinkedHashMap<>();
        for (int i = 0; i < pairs.length; i += 2) {
            map.put((String) pairs[i], (Integer) pairs[i + 1]);
        }
        return map;
    }

    @Test
    void reportsWhatAppeared() {
        List<BackpackDiff.Gain> gains = BackpackDiff.gains(
                counts("minecraft:bread", 2),
                counts("minecraft:bread", 2, "minecraft:iron_ingot", 3),
                counts());
        assertEquals(List.of(new BackpackDiff.Gain("minecraft:iron_ingot", 3)), gains);
    }

    @Test
    void reportsOnlyTheIncrease() {
        List<BackpackDiff.Gain> gains = BackpackDiff.gains(
                counts("minecraft:iron_ingot", 1),
                counts("minecraft:iron_ingot", 5),
                counts());
        assertEquals(List.of(new BackpackDiff.Gain("minecraft:iron_ingot", 4)), gains);
    }

    @Test
    void takingThingsOutIsSilent() {
        assertTrue(BackpackDiff.gains(
                counts("minecraft:iron_ingot", 5), counts("minecraft:iron_ingot", 2), counts()).isEmpty());
        assertTrue(BackpackDiff.gains(
                counts("minecraft:bread", 1), counts(), counts()).isEmpty());
    }

    @Test
    void pickupsAlreadyReportedAreNotRepeated() {
        assertTrue(BackpackDiff.gains(
                counts(), counts("minecraft:iron_ingot", 3), counts("minecraft:iron_ingot", 3)).isEmpty());
    }

    @Test
    void onlyTheUnreportedPartIsLeft() {
        List<BackpackDiff.Gain> gains = BackpackDiff.gains(
                counts(),
                counts("minecraft:iron_ingot", 5),
                counts("minecraft:iron_ingot", 3));
        assertEquals(List.of(new BackpackDiff.Gain("minecraft:iron_ingot", 2)), gains);
    }

    @Test
    void explanationOfAnotherItemDoesNotHideThisOne() {
        List<BackpackDiff.Gain> gains = BackpackDiff.gains(
                counts(),
                counts("minecraft:iron_ingot", 2, "minecraft:bread", 1),
                counts("minecraft:bread", 1));
        assertEquals(List.of(new BackpackDiff.Gain("minecraft:iron_ingot", 2)), gains);
    }

    @Test
    void orderIsStableForNarration() {
        List<BackpackDiff.Gain> gains = BackpackDiff.gains(
                counts(),
                counts("minecraft:wheat", 1, "minecraft:apple", 2),
                counts());
        assertEquals(List.of(
                new BackpackDiff.Gain("minecraft:apple", 2),
                new BackpackDiff.Gain("minecraft:wheat", 1)), gains);
    }
}
