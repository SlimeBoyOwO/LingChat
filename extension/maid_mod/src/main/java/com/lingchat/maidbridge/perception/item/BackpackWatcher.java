package com.lingchat.maidbridge.perception.item;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
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
 * 她的背包多了什么。玩家把东西给她的正路是打开女仆界面放进背包，那一路没有事件可挂，
 * 所以挂在感知的采样点上前后比一次（判定是 {@link BackpackDiff} 的纯逻辑）。
 *
 * <p>捡取另有精确事件，背包差分把那一份扣掉，同一件事只说一遍。
 *
 * <p>比的是「双手 + 背包」的总数，不是只看背包：她自己吃饭时整摞食物会被挪到手上，
 * 只看背包会把它当成刚收到（见 {@link #sample}）。
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

    private static Map<String, Integer> last;
    private static int lastSlots = -1;
    private static String lastMaid = "";

    private BackpackWatcher() {
    }

    /** 在一个采样点上调用。 */
    public static void sample(EntityMaid maid, BridgeServer bridge) {
        // 连双手一起看：她自己吃饭时车万女仆会把整摞食物从背包挪到手上，只看背包
        // 会先把那一摞当成「没了」，吃完放回来又当成「刚收到」——于是每吃一口都报
        // 一次「背包里多了 ×剩下全部」。总数里带上手，吃饭前后才是连续的。
        IItemHandler inventory = maid.getAvailableInv(true);
        Map<String, Integer> now = read(inventory);
        Map<String, Integer> explained = ItemReporter.drainRecentPickups();
        String maidId = maid.getStringUUID();
        // 换了监控对象与外挂背包都只重建基线：否则会把她的全部家当当成「刚收到」
        if (last == null || lastSlots != inventory.getSlots() || !lastMaid.equals(maidId)) {
            last = now;
            lastSlots = inventory.getSlots();
            lastMaid = maidId;
            return;
        }
        List<BackpackDiff.Gain> gains = BackpackDiff.gains(last, now, explained);
        last = now;
        if (gains.isEmpty()) {
            return;
        }
        JsonObject frame = EventFrames.base("item_received", maid.getStringUUID());
        JsonArray items = new JsonArray();
        for (BackpackDiff.Gain gain : gains) {
            JsonObject one = new JsonObject();
            one.addProperty("item", gain.item());
            one.addProperty("count", gain.count());
            items.add(one);
        }
        frame.add("items", items);
        // 交接只有「打开女仆界面放进背包」这一条没有事件可挂；界面对面必须站着主人。
        // 主人在旁边就按他放的来说，否则只说「背包里多了」——不替别人认领。
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
        lastSlots = -1;
        lastMaid = "";
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
