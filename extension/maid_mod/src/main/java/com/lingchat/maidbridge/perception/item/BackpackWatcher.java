package com.lingchat.maidbridge.perception.item;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.action.LongTaskGuard;
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
 * 她身上多了什么、少了什么。玩家在女仆界面里放进、拿出的正路都没有事件可挂，所以挂在
 * 感知的采样点上前后比一次（判定是 {@link BackpackDiff} 的纯逻辑）。
 *
 * <p>看的是整只女仆：双手、盔甲栏、背包、饰品栏。原来只比「双手 + 背包」，往盔甲栏里
 * 塞的护甲、往饰品栏里挂的饰品她一概不知道——玩家把铁镐放进主手，她也像没发生过一样。
 *
 * <p>先比总数、再判位置：{@link #sample} 的总数没变就说明只是在她身上挪了个位置
 * （她自己吃饭、换工具都会把东西从背包挪到手上），那种不算增减。临时寄存栏也算进来：
 * 车万女仆自己吃饭、喝水时会把手里那件暂时收进隐藏的一格，吃完再放回手上；不算进来的话，
 * 那一顿饭的时间里她的东西就「少了」，而主人往往正站在旁边看着。
 *
 * <p>捡取、她自己吃掉与交给主人的那些另有精确事件，差分把那一份扣掉，同一件事只说一遍。
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
            // 同一处可能由几个栏拼成（手 + 临时寄存栏），按位置名合并
            Map<String, Integer> partCounts =
                    nowParts.computeIfAbsent(part.slot(), key -> new LinkedHashMap<>());
            for (Map.Entry<String, Integer> entry : counts.entrySet()) {
                partCounts.merge(entry.getKey(), entry.getValue(), Integer::sum);
                now.merge(entry.getKey(), entry.getValue(), Integer::sum);
            }
            slots += part.handler().getSlots();
        }
        Map<String, Integer> explained = ItemReporter.drainRecentPickups();
        Map<String, Integer> ownGone = ItemReporter.drainRecentRemovals();
        String maidId = maid.getStringUUID();
        // 换了监控对象与外挂背包都只重建基线：否则会把她的全部家当当成「刚收到」
        if (last == null || lastSlots != slots || !lastMaid.equals(maidId)) {
            remember(now, nowParts, slots, maidId);
            return;
        }
        List<BackpackDiff.Gain> gains = BackpackDiff.gains(last, now, explained);
        List<BackpackDiff.Loss> losses = BackpackDiff.losses(last, now, ownGone);
        Map<String, Map<String, Integer>> before = lastParts;
        remember(now, nowParts, slots, maidId);
        if (gains.isEmpty() && losses.isEmpty()) {
            return;
        }
        // 交接只有「打开女仆界面」这一条没有事件可挂；界面对面必须站着主人。
        // 主人在旁边就按他放的、他拿的来说，否则只说「身上多了」——不替别人认领。
        ServerPlayer owner = MaidQueries.owner(maid);
        boolean ownerNear = owner != null
                && owner.level() == maid.level()
                && maid.distanceToSqr(owner) <= OWNER_ATTRIBUTE_DISTANCE * OWNER_ATTRIBUTE_DISTANCE;
        // 她自己破坏方块之后的增量归她自己，不是谁放进来的：我们的挖矿与车万女仆的干活
        // （农场、甘蔗、清雪……）都把掉落直接塞进她的库存，不走拾取事件。主人在旁边看她
        // 砍树、收菜时，原来每一份收成都被说成「主人往背包里放了木头 / 小麦」。
        // 挖矿由 maid_mine_done 自己交代挖到了几块，干活的那一路不必另说。
        boolean selfGathered = MaidMineController.isRunning(maid)
                || MaidMineController.minedRecently(maid)
                || ItemReporter.gatheredRecently(maid);
        if (!gains.isEmpty() && !selfGathered) {
            JsonObject frame = EventFrames.base("item_received", maidId);
            JsonArray items = new JsonArray();
            for (BackpackDiff.Gain gain : gains) {
                JsonObject one = new JsonObject();
                one.addProperty("item", gain.item());
                one.addProperty("count", gain.count());
                one.addProperty("slot", slotOf(gain.item(), before, nowParts, true));
                items.add(one);
            }
            frame.add("items", items);
            frame.addProperty("from_owner", ownerNear);
            bridge.broadcastEvent(frame);
        }
        // 拿走：她自己在干活的那段背包本来就会少东西（播种吃种子、拉弓吃箭），谁也说不清
        // 是被拿走了还是用掉了，所以只在主人贴身、她又闲着时才认这个账。
        if (!losses.isEmpty() && ownerNear && !selfGathered && !LongTaskGuard.active(maid)) {
            JsonObject frame = EventFrames.base("item_taken", maidId);
            JsonArray items = new JsonArray();
            for (BackpackDiff.Loss loss : losses) {
                JsonObject one = new JsonObject();
                one.addProperty("item", loss.item());
                one.addProperty("count", loss.count());
                one.addProperty("slot", slotOf(loss.item(), before, nowParts, false));
                items.add(one);
            }
            frame.add("items", items);
            bridge.broadcastEvent(frame);
        }
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
                // 她吃饭、喝水时手里那件会暂时收到这一格里，与手算同一处
                new Part("hand", maid.getHideInv()),
                new Part("armor", maid.getArmorInvWrapper()),
                new Part("bauble", maid.getMaidBauble()),
                new Part("backpack", maid.getAvailableBackpackInv()));
    }

    /**
     * 这件东西是从哪一处多出来（{@code gain}）或从哪一处少掉的：只有这一处朝那个方向动了
     * 才算在那里。
     *
     * <p>总数已经确认变了，找不到对应位置时按背包算——那是玩家最常放东西的地方。
     */
    private static String slotOf(
            String item, Map<String, Map<String, Integer>> before,
            Map<String, Map<String, Integer>> after, boolean gain) {
        String best = "backpack";
        int bestDelta = 0;
        for (Map.Entry<String, Map<String, Integer>> entry : after.entrySet()) {
            Map<String, Integer> old = before == null ? Map.of() : before.getOrDefault(entry.getKey(), Map.of());
            int delta = entry.getValue().getOrDefault(item, 0) - old.getOrDefault(item, 0);
            int moved = gain ? delta : -delta;
            if (moved > bestDelta) {
                bestDelta = moved;
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
