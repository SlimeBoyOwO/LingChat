package com.lingchat.maidbridge.action.give;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.bridge.Protocol;
import com.lingchat.maidbridge.maid.MaidCondition;
import com.lingchat.maidbridge.maid.MaidInventory;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.item.ItemReporter;
import java.util.Locale;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.neoforged.neoforge.items.wrapper.CombinedInvWrapper;

/**
 * {@code maid_give_item}：把她背包里的东西交给主人。
 *
 * <p>同步完成，不做成动作循环：从背包里扣掉指定的几件，直接塞进玩家背包，塞不下的掉在
 * 玩家脚下。玩家不在同一个世界、或者离得太远时就如实拒绝——她不能隔空把东西递过去。
 *
 * <p>只认她背包里的东西（玩家放进去的、她捡起来的）；她手上拿着的工具不算，那通常是
 * 她正在用来干活的那一把。拿不出来时回执里说明原因，插件原样转给模型。
 */
public class MaidGiveItemHandler implements MessageHandler {
    /** 超过这么远就不递了：她就在主人身边活动，够不着时让玩家先走近。 */
    private static final double GIVE_RANGE = 8.0;
    private static final int MAX_COUNT = 64;
    /** 失败回执与「看一眼」里最多列几种背包物品：模型只需要一个能下手的清单。 */
    private static final int AVAILABLE_LIMIT = 12;

    private final MinecraftServer server;

    public MaidGiveItemHandler(MinecraftServer server) {
        this.server = server;
    }

    @Override
    public JsonObject handle(JsonObject request) {
        if (server == null) {
            return Envelope.error(MaidQueries.NOT_READY);
        }
        JsonObject data = Envelope.data(request);
        EntityMaid maid = MaidQueries.findMaidById(server, Envelope.string(data, "maid_id"));
        if (maid == null) {
            return rejected("maid_not_found", MaidQueries.NOT_FOUND, "", 0, 0);
        }
        if (MaidCondition.isUnconscious(maid)) {
            return rejected("maid_unconscious", "她昏迷不醒，拿不了东西", "", 0, 0);
        }
        String itemId = parseId(Envelope.string(data, "item"));
        ResourceLocation id = itemId == null ? null : ResourceLocation.tryParse(itemId);
        Item item = id == null ? null : BuiltInRegistries.ITEM.get(id);
        if (item == null || item == Items.AIR) {
            // 认不出具体物品时把背包清单带回去：模型能按 id 再来一次，而不是干猜
            return rejected(maid, "bad_item", "没听懂要给的是哪件东西", "", 0, 0);
        }
        int wanted = parseCount(data);
        ServerPlayer owner = MaidQueries.owner(maid);
        if (owner == null || owner.level() != maid.level()) {
            return rejected("player_away", "玩家不在这里，没法把东西交给他", itemId, wanted, 0);
        }
        if (maid.distanceToSqr(owner) > GIVE_RANGE * GIVE_RANGE) {
            return rejected("too_far", "玩家离得有点远，先走近一点", itemId, wanted, 0);
        }

        CombinedInvWrapper backpack = maid.getAvailableBackpackInv();
        int remaining = wanted;
        int given = 0;
        for (int slot = 0; slot < backpack.getSlots() && remaining > 0; slot++) {
            // 头饰栏里的东西顶在头上，不算背包存货，交不出去
            if (slot == MaidInventory.HEADWEAR_SLOT) {
                continue;
            }
            ItemStack stack = backpack.getStackInSlot(slot);
            if (stack.isEmpty() || !stack.is(item)) {
                continue;
            }
            ItemStack taken = backpack.extractItem(slot, Math.min(remaining, stack.getCount()), false);
            if (taken.isEmpty()) {
                continue;
            }
            given += taken.getCount();
            remaining -= taken.getCount();
            if (!owner.getInventory().add(taken)) {
                owner.drop(taken, false);
            }
        }
        if (given == 0) {
            return rejected(maid, "not_found", "她身上没有这件东西", itemId, wanted, 0);
        }
        // 这几件是她自己递出去的：背包差分会看到背包少了，别让旁白说成「主人从背包里拿走了」
        ItemReporter.markRemoval(itemId, given);

        JsonObject out = new JsonObject();
        out.addProperty("accepted", true);
        out.addProperty("item", itemId);
        out.addProperty("name", new ItemStack(item).getHoverName().getString());
        out.addProperty("count", given);
        out.addProperty("requested", wanted);
        out.addProperty("reason_code", "");
        out.addProperty("reason", "");
        return Envelope.response(Protocol.MAID_GIVE_ITEM_RESULT, out);
    }

    /** 插件给的已经是注册表 id；短名补上原版命名空间，认不出给 null。 */
    private static String parseId(String raw) {
        String text = raw == null ? "" : raw.trim().toLowerCase(Locale.ROOT);
        if (text.isEmpty()) {
            return null;
        }
        ResourceLocation id = ResourceLocation.tryParse(text.contains(":") ? text : "minecraft:" + text);
        return id == null ? null : id.toString();
    }

    private static int parseCount(JsonObject data) {
        try {
            return Math.max(1, Math.min(MAX_COUNT, data.get("count").getAsInt()));
        } catch (RuntimeException missingOrNotANumber) {
            return 1;
        }
    }

    private static JsonObject rejected(String code, String reason, String item, int requested, int given) {
        JsonObject out = new JsonObject();
        out.addProperty("accepted", false);
        out.addProperty("item", item == null ? "" : item);
        out.addProperty("name", "");
        out.addProperty("count", given);
        out.addProperty("requested", requested);
        out.addProperty("reason_code", code);
        out.addProperty("reason", reason);
        return Envelope.response(Protocol.MAID_GIVE_ITEM_RESULT, out);
    }

    /** 认不出/找不到时多带一份背包清单，模型这一次往返就能改口。 */
    private static JsonObject rejected(
            EntityMaid maid, String code, String reason, String item, int requested, int given) {
        JsonObject out = rejected(code, reason, item, requested, given);
        Envelope.data(out).add("available", MaidInventory.stacks(maid, AVAILABLE_LIMIT));
        return out;
    }
}
