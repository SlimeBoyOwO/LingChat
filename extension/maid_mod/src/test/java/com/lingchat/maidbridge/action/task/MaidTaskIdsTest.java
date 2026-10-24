package com.lingchat.maidbridge.action.task;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import net.minecraft.resources.ResourceLocation;
import org.junit.jupiter.api.Test;

/**
 * 工作模式 id 的归一：模型给短名、完整 id、带空格或大小写混写都得认出来。
 *
 * <p>不认识的一律给 null——端口没鉴权，谁都能往这里塞字符串，而真正的白名单在
 * {@code TaskManager} 里，这里只负责把写法收拢。
 */
class MaidTaskIdsTest {

    @Test
    void shortNamesGetTheMaidNamespace() {
        assertEquals(ResourceLocation.parse("touhou_little_maid:farm"), MaidTaskIds.resolve("farm"));
        assertEquals(ResourceLocation.parse("touhou_little_maid:farm"), MaidTaskIds.resolve("touhou_little_maid:farm"));
    }

    @Test
    void fullIdsAndWhitespaceAreNormalized() {
        assertEquals(ResourceLocation.parse("touhou_little_maid:fishing"), MaidTaskIds.resolve("  TOUHOU_LITTLE_MAID:FISHING "));
    }

    @Test
    void otherNamespacesAreKeptForAddonTasks() {
        assertEquals(ResourceLocation.parse("kubejs:my_task"), MaidTaskIds.resolve("kubejs:my_task"));
    }

    @Test
    void garbageIsRejected() {
        assertNull(MaidTaskIds.resolve(""));
        assertNull(MaidTaskIds.resolve("   "));
        assertNull(MaidTaskIds.resolve(null));
        assertNull(MaidTaskIds.resolve("bad id!"));
        assertNull(MaidTaskIds.resolve("touhou_little_maid:"));
    }
}
