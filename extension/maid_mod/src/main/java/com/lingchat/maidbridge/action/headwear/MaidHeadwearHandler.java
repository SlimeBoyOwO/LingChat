package com.lingchat.maidbridge.action.headwear;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.bridge.Protocol;
import com.lingchat.maidbridge.maid.MaidCondition;
import com.lingchat.maidbridge.maid.MaidInventory;
import com.lingchat.maidbridge.maid.MaidQueries;
import java.util.Locale;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.neoforged.neoforge.items.ItemStackHandler;

/**
 * {@code maid_headwear}：把背包里的一件东西戴到她头上，或者摘下来。
 *
 * <p>她背包的最后一格（{@link MaidInventory#HEADWEAR_SLOT}）是头饰栏，那一格里的东西
 * 顶在头上显示——放一块方块就是顶着一块方块。这一格不在「背包存货」里：交东西、换工具
 * 都不会动它，{@code maid_give_item} 也拿不到它。
 *
 * <p>换下来的旧头饰必须有地方放（刚腾出来的那格或任一空格），否则整件事撤掉，不能把
 * 东西凭空弄丢。
 */
public class MaidHeadwearHandler implements MessageHandler {
    private static final String TAKE_OFF = "take_off";

    private final MinecraftServer server;

    public MaidHeadwearHandler(MinecraftServer server) {
        this.server = server;
    }

    @Override
    public JsonObject handle(JsonObject request) {
        if (server == null) {
            return Envelope.error(MaidQueries.NOT_READY);
        }
        EntityMaid maid = MaidQueries.findMaidById(server, Envelope.string(Envelope.data(request), "maid_id"));
        if (maid == null) {
            return rejected("maid_not_found", MaidQueries.NOT_FOUND, "", "");
        }
        if (MaidCondition.isUnconscious(maid)) {
            return rejected("maid_unconscious", "她昏迷不醒，戴不了东西", "", "");
        }
        JsonObject data = Envelope.data(request);
        String rawItem = Envelope.string(data, "item").trim();
        String action = Envelope.string(data, "action").trim().toLowerCase(Locale.ROOT);
        if (TAKE_OFF.equals(action) || rawItem.isEmpty() || "none".equalsIgnoreCase(rawItem)) {
            return takeOff(maid);
        }
        ItemStackHandler inventory = maid.getMaidInv();
        String itemId = parseId(rawItem);
        Item item = itemId == null ? null : BuiltInRegistries.ITEM.get(ResourceLocation.parse(itemId));
        if (item == null || item == Items.AIR) {
            return rejected("bad_item", "没听懂要戴上的是哪件东西", "", current(maid));
        }
        int source = findItem(inventory, item);
        if (source < 0) {
            return rejected("not_found", "她背包里没有这件东西", itemId, current(maid));
        }
        ItemStack wearing = inventory.getStackInSlot(MaidInventory.HEADWEAR_SLOT);
        ItemStack one = inventory.getStackInSlot(source).copyWithCount(1);
        if (!EntityMaid.canInsertItem(one)) {
            return rejected("not_allowed", "这个东西顶不到她头上", itemId, current(maid));
        }
        // 换下来的旧头饰得有地方放：优先用刚腾出来的那格（整摞拿光时它就空了）
        int spare = -1;
        if (!wearing.isEmpty()) {
            spare = inventory.getStackInSlot(source).getCount() == 1 ? source : emptySlot(inventory);
            if (spare < 0) {
                return rejected("no_space", "背包里没有空位放下换下来的东西", itemId, current(maid));
            }
        }
        ItemStack taken = inventory.extractItem(source, 1, false);
        if (!wearing.isEmpty()) {
            ItemStack rest = inventory.insertItem(spare, wearing, false);
            if (!rest.isEmpty()) {
                inventory.insertItem(source, taken, false);
                return rejected("no_space", "背包里没有空位放下换下来的东西", itemId, current(maid));
            }
        }
        inventory.setStackInSlot(MaidInventory.HEADWEAR_SLOT, taken);
        return accepted(maid, "put", taken);
    }

    private static JsonObject takeOff(EntityMaid maid) {
        ItemStackHandler inventory = maid.getMaidInv();
        ItemStack wearing = inventory.getStackInSlot(MaidInventory.HEADWEAR_SLOT);
        if (wearing.isEmpty()) {
            return rejected("empty_head", "她头上没有戴东西", "", "");
        }
        int spare = emptySlot(inventory);
        if (spare < 0) {
            return rejected("no_space", "背包满了，摘下来没处放", "", current(maid));
        }
        inventory.setStackInSlot(MaidInventory.HEADWEAR_SLOT, ItemStack.EMPTY);
        inventory.setStackInSlot(spare, wearing.copy());
        return accepted(maid, TAKE_OFF, wearing);
    }

    /** 背包里第一格这件东西；头饰栏本身不算存货。 */
    private static int findItem(ItemStackHandler inventory, Item item) {
        for (int slot = 0; slot < inventory.getSlots(); slot++) {
            if (slot == MaidInventory.HEADWEAR_SLOT) {
                continue;
            }
            ItemStack stack = inventory.getStackInSlot(slot);
            if (!stack.isEmpty() && stack.is(item)) {
                return slot;
            }
        }
        return -1;
    }

    private static int emptySlot(ItemStackHandler inventory) {
        for (int slot = 0; slot < inventory.getSlots(); slot++) {
            if (slot != MaidInventory.HEADWEAR_SLOT && inventory.getStackInSlot(slot).isEmpty()) {
                return slot;
            }
        }
        return -1;
    }

    private static String current(EntityMaid maid) {
        ItemStack wearing = MaidInventory.headwear(maid);
        return wearing.isEmpty() ? "" : BuiltInRegistries.ITEM.getKey(wearing.getItem()).toString();
    }

    private static String parseId(String raw) {
        String text = raw.trim().toLowerCase(Locale.ROOT);
        ResourceLocation id = ResourceLocation.tryParse(text.contains(":") ? text : "minecraft:" + text);
        return id == null ? null : id.toString();
    }

    private static JsonObject accepted(EntityMaid maid, String action, ItemStack stack) {
        JsonObject out = new JsonObject();
        out.addProperty("accepted", true);
        out.addProperty("action", action);
        out.addProperty("item", BuiltInRegistries.ITEM.getKey(stack.getItem()).toString());
        out.addProperty("name", stack.getHoverName().getString());
        out.addProperty("headwear", current(maid));
        out.addProperty("reason_code", "");
        out.addProperty("reason", "");
        return Envelope.response(Protocol.MAID_HEADWEAR_RESULT, out);
    }

    private static JsonObject rejected(String code, String reason, String item, String headwear) {
        JsonObject out = new JsonObject();
        out.addProperty("accepted", false);
        out.addProperty("action", "");
        out.addProperty("item", item == null ? "" : item);
        out.addProperty("name", "");
        out.addProperty("headwear", headwear == null ? "" : headwear);
        out.addProperty("reason_code", code);
        out.addProperty("reason", reason);
        return Envelope.response(Protocol.MAID_HEADWEAR_RESULT, out);
    }
}
