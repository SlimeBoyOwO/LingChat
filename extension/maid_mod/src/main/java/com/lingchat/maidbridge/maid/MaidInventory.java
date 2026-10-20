package com.lingchat.maidbridge.maid;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.item.ItemStack;
import net.neoforged.neoforge.items.wrapper.CombinedInvWrapper;

/** 她背包里有什么。只读清单，交物品与「看一眼周围」共用。 */
public final class MaidInventory {
    private MaidInventory() {
    }

    /** 她背包里能交出去的东西：同类合并、最多 limit 种。 */
    public static JsonArray stacks(EntityMaid maid, int limit) {
        Map<String, Integer> totals = new LinkedHashMap<>();
        CombinedInvWrapper backpack = maid.getAvailableBackpackInv();
        for (int slot = 0; slot < backpack.getSlots(); slot++) {
            ItemStack stack = backpack.getStackInSlot(slot);
            if (stack.isEmpty()) {
                continue;
            }
            String id = BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
            if (!totals.containsKey(id) && totals.size() >= limit) {
                continue;
            }
            totals.merge(id, stack.getCount(), Integer::sum);
        }
        JsonArray out = new JsonArray();
        for (Map.Entry<String, Integer> entry : totals.entrySet()) {
            JsonObject one = new JsonObject();
            one.addProperty("item", entry.getKey());
            one.addProperty("count", entry.getValue());
            out.add(one);
        }
        return out;
    }
}
