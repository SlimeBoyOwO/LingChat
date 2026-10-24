package com.lingchat.maidbridge.perception.item;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.action.mine.MaidMineController;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.EventFrames;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.item.ItemStack;
import net.neoforged.neoforge.items.IItemHandler;

/**
 * 她身上多了什么。玩家把东西给她的正路是打开女仆界面放进背包，那一路没有事件可挂，
 * 所以挂在感知的采样点上前后比一次（判定是 {@link BackpackDiff} 的纯逻辑）。
 *
 * <p>看的是整只女仆：双手、盔甲栏、背包、饰品栏。原来只比「双手 + 背包」，往盔甲栏里
 * 塞的护甲、往饰品栏里挂的饰品她一概不知道——玩家把铁镐放进主手，她也像没发生过一样。
 *
 * <p>先比总数、再判位置：{@link #sample} 的总数没变就说明只是在她身上挪了个位置
 * （她自己吃饭、换工具都会把东西从背包挪到手上），那种不算「收到东西」。
 *
 * <p>捡取另有精确事件，背包差分把那一份扣掉，同一件事只说一遍。
 *
 * <p>首帧、换了监控对象、换了背包（格子数变了）、关掉「身边的变化」都只重建基线，
 * 不出事件——否则刚接上就会把她的全部家当报一遍。
 */
public final class BackpackWatcher {
    /**
     * 主人在这个距离内就把「背包里多了」说成「主人放进来的」：女仆界面要贴身才打得开，
     * 这个范围内没有第二条路能把东西塞进她的背包。
     */
    private static final double OWNER_ATTRIBUTE_DISTANCE = 8.0;

    /** 她身上能放东西的几处，顺序即位置标记的优先级（同一件东西几处都增加时取靠前的）。 */
    private record Part(String slot, IItemHandler handler) {
    }

    private static Map<String, Integer> last;
    private static Map<String, Map<String, Integer>> lastParts;
    private static int lastSlots = -1;
    private static String lastMaid = "";

    private BackpackWatcher() {
    }

    /** 在一个采样点上调用。 */
    public static void sample(EntityMaid maid, BridgeServer bridge) {
        List<Part> parts = parts(maid);
        Map<String, Map<String, Integer>> nowParts = new LinkedHashMap<>();
        Map<String, Integer> now = new LinkedHashMap<>();
        int slots = 0;
        for (Part part : parts) {
            Map<String, Integer> counts = read(part.handler());
            nowParts.put(part.slot(), counts);
            for (Map.Entry<String, Integer> entry : counts.entrySet()) {
                now.merge(entry.getKey(), entry.getValue(), Integer::sum);
            }
            slots += part.handler().getSlots();
        }
        Map<String, Integer> explained = ItemReporter.drainRecentPickups();
        String maidId = maid.getStringUUID();
        // 换了监控对象与外挂背包都只重建基线：否则会把她的全部家当当成「刚收到」
        if (last == null || lastSlots != slots || !lastMaid.equals(maidId)) {
            remember(now, nowParts, slots, maidId);
            return;
        }
        List<BackpackDiff.Gain> gains = BackpackDiff.gains(last, now, explained);
        Map<String, Map<String, Integer>> before = lastParts;
        remember(now, nowParts, slots, maidId);
        if (gains.isEmpty()) {
            return;
        }
        // 她自己挖下来的东西直接进背包（车万女仆的 destroyBlock 把掉落塞进她的库存），
        // 不走拾取事件。挖着和刚挖完的这一小段里，增量都是挖出来的，不是谁放进来的：
        // 主人在旁边看她砍树时，原来每一块木头都被说成「主人往背包里放了木头」。
        // 挖掘收尾那条（maid_mine_done）会自己交代挖到了几块。
        if (MaidMineController.isRunning(maid) || MaidMineController.minedRecently(maid)) {
            return;
        }
        JsonObject frame = EventFrames.base("item_received", maidId);
        JsonArray items = new JsonArray();
        for (BackpackDiff.Gain gain : gains) {
            JsonObject one = new JsonObject();
            one.addProperty("item", gain.item());
            one.addProperty("count", gain.count());
            one.addProperty("slot", slotOf(gain.item(), before, nowParts));
            items.add(one);
        }
        frame.add("items", items);
        // 交接只有「打开女仆界面放进背包」这一条没有事件可挂；界面对面必须站着主人。
        // 主人在旁边就按他放的来说，否则只说「身上多了」——不替别人认领。
        ServerPlayer owner = MaidQueries.owner(maid);
        boolean ownerNear = owner != null
                && owner.level() == maid.level()
                && maid.distanceToSqr(owner) <= OWNER_ATTRIBUTE_DISTANCE * OWNER_ATTRIBUTE_DISTANCE;
        frame.addProperty("from_owner", ownerNear);
        bridge.broadcastEvent(frame);
    }

    /** 下次采样只重建基线。她离开已加载区块、换监控对象、关掉开关时都要清。 */
    public static void clear() {
        last = null;
        lastParts = null;
        lastSlots = -1;
        lastMaid = "";
    }

    private static void remember(
            Map<String, Integer> now, Map<String, Map<String, Integer>> nowParts, int slots, String maidId) {
        last = now;
        lastParts = nowParts;
        lastSlots = slots;
        lastMaid = maidId;
    }

    private static List<Part> parts(EntityMaid maid) {
        return List.of(
                new Part("hand", maid.getHandsInvWrapper()),
                new Part("armor", maid.getArmorInvWrapper()),
                new Part("bauble", maid.getMaidBauble()),
                new Part("backpack", maid.getAvailableBackpackInv()));
    }

    /**
     * 这件东西是从哪一处多出来的：只在这一处涨了、别处没跌，才算放在那里。
     *
     * <p>总数已经确认是「多了」，所以找不到涨的位置时按背包算——那是玩家最常放东西的地方。
     */
    private static String slotOf(
            String item, Map<String, Map<String, Integer>> before, Map<String, Map<String, Integer>> after) {
        String best = "backpack";
        int bestDelta = 0;
        for (Map.Entry<String, Map<String, Integer>> entry : after.entrySet()) {
            Map<String, Integer> old = before == null ? Map.of() : before.getOrDefault(entry.getKey(), Map.of());
            int delta = entry.getValue().getOrDefault(item, 0) - old.getOrDefault(item, 0);
            if (delta > bestDelta) {
                bestDelta = delta;
                best = entry.getKey();
            }
        }
        return best;
    }

    private static Map<String, Integer> read(IItemHandler inventory) {
        Map<String, Integer> counts = new LinkedHashMap<>();
        for (int i = 0; i < inventory.getSlots(); i++) {
            ItemStack stack = inventory.getStackInSlot(i);
            if (stack.isEmpty()) {
                continue;
            }
            String item = BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
            counts.merge(item, stack.getCount(), Integer::sum);
        }
        return counts;
    }
}
