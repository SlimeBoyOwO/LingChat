package com.lingchat.maidbridge.perception.nearby;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import org.junit.jupiter.api.Test;

/** 按种类记的冷静期：牧场里同一只羊不该反复被提，种类多了也不该把表撑大。 */
class KeyedCooldownTest {

    private static final long COOLDOWN = 1200L;

    @Test
    void theFirstTimeIsAlwaysAllowed() {
        assertTrue(new KeyedCooldown().allow("minecraft:sheep", 500L, COOLDOWN));
    }

    @Test
    void theSameKeyIsBlockedUntilTheCooldownPasses() {
        KeyedCooldown cooldown = new KeyedCooldown();
        assertTrue(cooldown.allow("minecraft:sheep", 500L, COOLDOWN), "第一次该允许");
        assertFalse(cooldown.allow("minecraft:sheep", 1699L, COOLDOWN), "还没到点");
        assertTrue(cooldown.allow("minecraft:sheep", 1700L, COOLDOWN), "到点就该放行");
    }

    @Test
    void beingBlockedDoesNotPushTheNextChanceBack() {
        KeyedCooldown cooldown = new KeyedCooldown();
        cooldown.allow("minecraft:sheep", 0L, COOLDOWN);
        assertFalse(cooldown.allow("minecraft:sheep", 100L, COOLDOWN));
        assertTrue(cooldown.allow("minecraft:sheep", COOLDOWN, COOLDOWN), "从上次报出去的那一刻算起");
    }

    @Test
    void eachKeyHasItsOwnClock() {
        KeyedCooldown cooldown = new KeyedCooldown();
        assertTrue(cooldown.allow("minecraft:sheep", 0L, COOLDOWN));
        assertTrue(cooldown.allow("minecraft:cow", 0L, COOLDOWN), "另一种活物不受它的冷静期影响");
    }

    /** 表有上限：待久了挤掉最久没提过的那个，于是它下次再来还能报一次。 */
    @Test
    void theOldestKeyIsForgottenWhenTheTableIsFull() {
        KeyedCooldown cooldown = new KeyedCooldown();
        for (int i = 0; i < 40; i++) {
            assertTrue(cooldown.allow("mob_" + i, 1000L, COOLDOWN), "第一次都该允许");
        }
        assertFalse(cooldown.allow("mob_39", 1000L, COOLDOWN), "刚提过的还在表里");
        assertTrue(cooldown.allow("mob_0", 1000L, COOLDOWN), "最早那个已经被挤掉了");
    }

    @Test
    void resetForgetsEverything() {
        KeyedCooldown cooldown = new KeyedCooldown();
        cooldown.allow("minecraft:sheep", 0L, COOLDOWN);
        cooldown.reset();
        assertTrue(cooldown.allow("minecraft:sheep", 1L, COOLDOWN), "换监控对象后重新算");
    }
}
