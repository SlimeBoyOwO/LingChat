package com.lingchat.maidbridge.maid;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.github.tartaricacid.touhoulittlemaid.inventory.handler.MaidBackpackHandler;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.neoforged.neoforge.items.wrapper.CombinedInvWrapper;

/** 她背包里有什么。只读清单，交物品与「看一眼周围」共用。 */
public final class MaidInventory {
    /**
     * 她的背包最后一格是头饰栏：那件东西顶在头上，不算背包里的存货。
     *
     * <p>交物品、换工具都不该动它，感知里也单独说，免得模型把头上的方块当成能交出去的东西。
     */
    public static final int HEADWEAR_SLOT = MaidBackpackHandler.BACKPACK_ITEM_SLOT;

    private MaidInventory() {
    }

    /** 她背包里能交出去的东西：同类合并、最多 limit 种。 */
    public static JsonArray stacks(EntityMaid maid, int limit) {
        Map<String, Integer> totals = new LinkedHashMap<>();
        CombinedInvWrapper backpack = maid.getAvailableBackpackInv();
        for (int slot = 0; slot < backpack.getSlots(); slot++) {
            if (slot == HEADWEAR_SLOT) {
                continue;
            }
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

    /** 头饰栏里现在是什么；空着给 {@link ItemStack#EMPTY}。 */
    public static ItemStack headwear(EntityMaid maid) {
        return maid.getMaidInv().getStackInSlot(HEADWEAR_SLOT);
    }

    /** 她手上、副手或背包里有没有这件东西；头饰栏不算存货。 */
    public static boolean has(EntityMaid maid, Item item) {
        if (maid.getMainHandItem().is(item) || maid.getOffhandItem().is(item)) {
            return true;
        }
        CombinedInvWrapper backpack = maid.getAvailableBackpackInv();
        for (int slot = 0; slot < backpack.getSlots(); slot++) {
            if (slot == HEADWEAR_SLOT) {
                continue;
            }
            if (backpack.getStackInSlot(slot).is(item)) {
                return true;
            }
        }
        return false;
    }

    /** 从她身上取走一件：先主手、再副手、最后背包。取到给 true。 */
    public static boolean takeOne(EntityMaid maid, Item item) {
        if (maid.getMainHandItem().is(item)) {
            maid.getMainHandItem().shrink(1);
            return true;
        }
        if (maid.getOffhandItem().is(item)) {
            maid.getOffhandItem().shrink(1);
            return true;
        }
        CombinedInvWrapper backpack = maid.getAvailableBackpackInv();
        for (int slot = 0; slot < backpack.getSlots(); slot++) {
            if (slot == HEADWEAR_SLOT) {
                continue;
            }
            if (backpack.getStackInSlot(slot).is(item)) {
                backpack.extractItem(slot, 1, false);
                return true;
            }
        }
        return false;
    }
}
