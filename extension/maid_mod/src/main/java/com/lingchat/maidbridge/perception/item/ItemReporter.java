package com.lingchat.maidbridge.perception.item;

import com.github.tartaricacid.touhoulittlemaid.api.event.MaidPickupEvent;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.MaidBridgeConfig;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.maid.MonitoredMaidHandler;
import com.lingchat.maidbridge.perception.EventFrames;
import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.neoforged.neoforge.event.entity.item.ItemTossEvent;
import net.neoforged.neoforge.event.entity.living.LivingEntityUseItemEvent;

/**
 * 物品的来去：玩家丢给她的、她捡起来的、喂给她吃的、她自己吃的。
 *
 * <p>三件事都只有游戏这一侧看得见（谁丢的、哪只是被监控的女仆、吃了什么），所以判定与
 * 去抖都在这里，中文措辞留给插件。丢与喂都是「一小段连续动作合成一条」：丢一背包东西、
 * 连喂几口时，台词表不该被刷屏。
 *
 * <p>玩家在女仆界面里把东西放进背包没有事件可挂，那一路由 {@link BackpackWatcher} 每轮
 * 采样比出来；它会把这里记下的捡取扣掉，同一件事只说一遍。
 */
public final class ItemReporter {
    /** 最后一下之后等这么久再报（tick）。 */
    private static final int DEBOUNCE_TICKS = 10;

    private static final Map<String, Integer> gifts = new LinkedHashMap<>();
    private static String giftMaid = "";
    private static double giftDistance;
    private static int giftCooldown;
    private static final Map<String, Integer> feeds = new LinkedHashMap<>();
    private static String feedMaid = "";
    private static int feedCooldown;
    /** 她自己从背包里找东西吃（车万女仆的进食系统），与玩家喂食分开说。 */
    private static final Map<String, Integer> selfFeeds = new LinkedHashMap<>();
    private static String selfFeedMaid = "";
    private static int selfFeedCooldown;
    /** 上一次采样之后她捡起来的物品：背包差分要把这部分扣掉。 */
    private static final Map<String, Integer> recentPickups = new LinkedHashMap<>();
    /** 这次捡的是不是主人丢的（Pre 与 Post 在同一次拾取调用里前后脚触发）。 */
    private static boolean pendingFromOwner;

    private ItemReporter() {
    }

    /** 主人把手里的东西丢了出去。 */
    public static void onToss(ItemTossEvent event) {
        Player player = event.getPlayer();
        if (player.level().isClientSide() || !reportsEnabled()) {
            return;
        }
        EntityMaid maid = monitoredMaid(player.getServer());
        if (maid == null || maid.getOwner() != player || maid.level() != player.level()) {
            return;
        }
        ItemEntity entity = event.getEntity();
        double distance = Math.sqrt(maid.distanceToSqr(entity));
        if (distance > MaidBridgeConfig.NOTICE_RADIUS.get()) {
            return;
        }
        ItemStack stack = entity.getItem();
        if (stack.isEmpty()) {
            return;
        }
        gifts.merge(itemId(stack), stack.getCount(), Integer::sum);
        giftMaid = maid.getStringUUID();
        // 丢了好几堆时按最近那堆说，插件只需要知道「就在身边」还是「隔了几步」
        giftDistance = giftCooldown == 0 ? distance : Math.min(giftDistance, distance);
        giftCooldown = DEBOUNCE_TICKS;
    }

    /** 拾取前的判定：这只物品是不是主人丢的。模拟拾取（只探能不能装下）不算。 */
    public static void onPickupPre(MaidPickupEvent.ItemResultPre event) {
        pendingFromOwner = false;
        if (event.isSimulate() || !reportsEnabled()) {
            return;
        }
        EntityMaid maid = event.getMaid();
        if (!isMonitored(maid)) {
            return;
        }
        Entity owner = maid.getOwner();
        ItemEntity item = event.getEntityItem();
        pendingFromOwner = owner != null && item != null && item.getOwner() == owner;
    }

    /** 她真的把东西收进背包了。别人的东西也报，只是「主人给的」才值得她当场开口（插件判）。 */
    public static void onPickupPost(MaidPickupEvent.ItemResultPost event) {
        boolean fromOwner = pendingFromOwner;
        pendingFromOwner = false;
        if (!reportsEnabled()) {
            return;
        }
        EntityMaid maid = event.getMaid();
        if (!isMonitored(maid)) {
            return;
        }
        ItemStack stack = event.getPickupItem();
        if (stack.isEmpty()) {
            return;
        }
        String item = itemId(stack);
        recentPickups.merge(item, stack.getCount(), Integer::sum);
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge == null || !bridge.hasClients()) {
            return;
        }
        JsonObject frame = EventFrames.base("item_pickup", maid.getStringUUID());
        frame.addProperty("item", item);
        frame.addProperty("count", stack.getCount());
        frame.addProperty("from_owner", fromOwner);
        bridge.broadcastEvent(frame);
    }

    /** 玩家刚喂了她一口。一次一口，连着喂由下面的去抖合并。 */
    public static void onFed(EntityMaid maid, ItemStack eaten) {
        if (!reportsEnabled() || eaten.isEmpty()) {
            return;
        }
        feeds.merge(itemId(eaten), 1, Integer::sum);
        feedMaid = maid.getStringUUID();
        feedCooldown = DEBOUNCE_TICKS;
    }

    /**
     * 她自己吃完了东西（车万女仆的进食系统会先 {@code startUsingItem}，完成时走这个事件）。
     *
     * <p>用「用完了哪件物品」来判定，而不是 {@code MaidAfterEatEvent}：后者只给吃完后剩下的
     * 那份，普通食物那里是空的，认不出吃的是什么。玩家喂食走的是直接 {@code eat}，不会
     * 经过这里，两条路不会重复。
     */
    public static void onItemUseFinish(LivingEntityUseItemEvent.Finish event) {
        if (!(event.getEntity() instanceof EntityMaid maid) || maid.level().isClientSide()) {
            return;
        }
        if (!reportsEnabled() || !isMonitored(maid)) {
            return;
        }
        ItemStack used = event.getItem();
        if (used.isEmpty() || used.getFoodProperties(maid) == null) {
            return;
        }
        selfFeeds.merge(itemId(used), 1, Integer::sum);
        selfFeedMaid = maid.getStringUUID();
        selfFeedCooldown = DEBOUNCE_TICKS;
    }

    /** 每个服务端 tick 调一次：去抖到点就把攒下的推出去。 */
    public static void tick(BridgeServer bridge) {
        if (giftCooldown > 0 && --giftCooldown == 0) {
            pushItems(bridge, "item_gift", giftMaid, gifts, true);
            gifts.clear();
            giftMaid = "";
        }
        if (feedCooldown > 0 && --feedCooldown == 0) {
            pushItems(bridge, "item_fed", feedMaid, feeds, false);
            feeds.clear();
            feedMaid = "";
        }
        if (selfFeedCooldown > 0 && --selfFeedCooldown == 0) {
            pushItems(bridge, "maid_ate", selfFeedMaid, selfFeeds, false);
            selfFeeds.clear();
            selfFeedMaid = "";
        }
    }

    /**
     * 取走并清空「上一次采样之后捡起来的物品」，供背包差分扣账。
     *
     * <p>采样与捡取都在主线程，不存在并发。
     */
    public static Map<String, Integer> drainRecentPickups() {
        Map<String, Integer> copy = new LinkedHashMap<>(recentPickups);
        recentPickups.clear();
        return copy;
    }

    /** 停服、换存档、没人听的时候清干净：别把上一波东西算到下一次头上。 */
    public static void clear() {
        gifts.clear();
        giftMaid = "";
        giftDistance = 0;
        giftCooldown = 0;
        feeds.clear();
        feedMaid = "";
        feedCooldown = 0;
        selfFeeds.clear();
        selfFeedMaid = "";
        selfFeedCooldown = 0;
        recentPickups.clear();
        pendingFromOwner = false;
    }

    private static void pushItems(BridgeServer bridge, String type, String maidId,
                                  Map<String, Integer> items, boolean withDistance) {
        if (items.isEmpty() || bridge == null || !bridge.hasClients()) {
            return;
        }
        JsonObject frame = EventFrames.base(type, maidId);
        JsonArray array = new JsonArray();
        for (Map.Entry<String, Integer> entry : items.entrySet()) {
            JsonObject one = new JsonObject();
            one.addProperty("item", entry.getKey());
            one.addProperty("count", entry.getValue());
            array.add(one);
        }
        frame.add("items", array);
        if (withDistance) {
            frame.addProperty("distance", EventFrames.round(giftDistance));
        }
        bridge.broadcastEvent(frame);
    }

    private static boolean reportsEnabled() {
        if (!MaidBridgeConfig.NOTICE_ENABLED.get()) {
            return false;
        }
        BridgeServer bridge = LingChatMaidBridge.server();
        return bridge != null && bridge.hasClients();
    }

    private static EntityMaid monitoredMaid(MinecraftServer server) {
        return MaidQueries.findMaidById(server, MonitoredMaidHandler.get());
    }

    private static boolean isMonitored(EntityMaid maid) {
        String monitored = MonitoredMaidHandler.get();
        return !monitored.isEmpty() && monitored.equals(maid.getStringUUID());
    }

    private static String itemId(ItemStack stack) {
        return BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
    }
}
