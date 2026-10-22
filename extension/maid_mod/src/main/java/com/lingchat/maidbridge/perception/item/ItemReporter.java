package com.lingchat.maidbridge.perception.item;

import com.github.tartaricacid.touhoulittlemaid.api.event.MaidPickupEvent;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.MaidBridgeConfig;
import com.lingchat.maidbridge.action.feed.MaidFeeder;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.maid.MaidCondition;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.maid.MonitoredMaidHandler;
import com.lingchat.maidbridge.perception.EventFrames;
import com.lingchat.maidbridge.perception.nearby.SurroundingsReader;
import java.util.ArrayDeque;
import java.util.Deque;
import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.Level;
import net.minecraft.world.phys.Vec3;
import net.neoforged.neoforge.event.entity.living.LivingEntityUseItemEvent;
import net.neoforged.neoforge.event.entity.living.LivingDeathEvent;
import net.neoforged.neoforge.event.level.BlockEvent;

/**
 * 物品的来去：她捡起来的、喂给她吃的、她自己吃的。
 *
 * <p>地上的东西不单独报：玩家随手丢下、怪物掉的、挖方块崩出来的，只要她没捡，就与她
 * 无关。真捡起来时才推一条，并说清来源——谁丢的（主人还是别的玩家）、哪只生物掉的、
 * 还是挖哪种方块崩出来的；认不出的只说「捡起了」。
 *
 * <p>喂食与自食仍然只有游戏这一侧看得见，所以判定与去抖都在这里，中文措辞留给插件。
 * 喂与吃都是「一小段连续动作合成一条」：连喂几口时，台词表不该被刷屏。
 *
 * <p>玩家在女仆界面里把东西放进背包没有事件可挂，那一路由 {@link BackpackWatcher} 每轮
 * 采样比出来；它会把这里记下的捡取扣掉，同一件事只说一遍。
 */
public final class ItemReporter {
    /** 喂食的去抖（tick）。 */
    private static final int DEBOUNCE_TICKS = 10;
    /**
     * 她自己吃饭的去抖：一口与下一口之间大约隔 32 tick，用喂食那套 10 tick 会把
     * 一顿饭拆成好几条旁白。改成「停下这么久才算吃完」，一顿只报一条 ×N。
     */
    private static final int MEAL_DEBOUNCE_TICKS = 120;
    /** 掉落来源记这么久（tick）：她可能过一会儿才走过去捡。 */
    private static final int SOURCE_MEMORY_TICKS = 600;
    /** 最多记最近几次死亡与破坏，够认她身边那几件东西就行。 */
    private static final int SOURCE_LIMIT = 32;
    /** 来源位置与捡到的物品相距这么近才算同一件事（格）。 */
    private static final double SOURCE_RADIUS = 3.0;

    /** 最近一次死亡或方块被破坏的位置：用来认地上那件东西是哪来的。 */
    private record Source(Level level, Vec3 pos, String kind, String id, long tick) {
    }

    /** 这件东西的来源：谁丢的、哪只生物掉的、哪种方块崩出来的。 */
    private record Origin(String kind, String id, String name) {
        static final Origin UNKNOWN = new Origin("unknown", "", "");
    }

    private static final Deque<Source> sources = new ArrayDeque<>();
    private static final Map<String, Integer> feeds = new LinkedHashMap<>();
    private static String feedMaid = "";
    private static int feedCooldown;
    /** 她自己从背包里找东西吃（车万女仆的进食系统），与玩家喂食分开说。 */
    private static final Map<String, Integer> selfFeeds = new LinkedHashMap<>();
    private static String selfFeedMaid = "";
    private static int selfFeedCooldown;
    /** 上一次采样之后她捡起来的物品：背包差分要把这部分扣掉。 */
    private static final Map<String, Integer> recentPickups = new LinkedHashMap<>();
    /** 这一次捡的东西是哪来的（Pre 与 Post 在同一次拾取调用里前后脚触发）。 */
    private static ItemEntity pendingItem;
    private static boolean pendingFromOwner;

    private ItemReporter() {
    }

    /** 有东西死了：记下位置与种类，之后她捡起附近的掉落物时就能说出「谁掉的」。 */
    public static void onDeath(LivingDeathEvent event) {
        if (!reportsEnabled()) {
            return;
        }
        LivingEntity dead = event.getEntity();
        if (dead instanceof EntityMaid || dead.level().isClientSide()) {
            return;
        }
        record(dead.level(), dead.position(), "mob", SurroundingsReader.typeId(dead));
    }

    /** 方块被玩家破坏：同上，用来认「挖方块崩出来的」。 */
    public static void onBlockBreak(BlockEvent.BreakEvent event) {
        if (!reportsEnabled() || !(event.getLevel() instanceof Level level)) {
            return;
        }
        String block = BuiltInRegistries.BLOCK.getKey(event.getState().getBlock()).toString();
        record(level, Vec3.atCenterOf(event.getPos()), "block", block);
    }

    /** 拾取前的判定：这件东西是谁丢的。模拟拾取（只探能不能装下）不算。 */
    public static void onPickupPre(MaidPickupEvent.ItemResultPre event) {
        pendingFromOwner = false;
        pendingItem = null;
        if (event.isSimulate() || !reportsEnabled()) {
            return;
        }
        EntityMaid maid = event.getMaid();
        if (!isMonitored(maid) || MaidCondition.isUnconscious(maid)) {
            return;
        }
        Entity owner = maid.getOwner();
        ItemEntity item = event.getEntityItem();
        pendingItem = item;
        pendingFromOwner = owner != null && item != null && item.getOwner() == owner;
    }

    /** 她真的把东西收进背包了。别人的东西也报，只是「主人给的」才值得她当场开口（插件判）。 */
    public static void onPickupPost(MaidPickupEvent.ItemResultPost event) {
        boolean fromOwner = pendingFromOwner;
        ItemEntity picked = pendingItem;
        pendingFromOwner = false;
        pendingItem = null;
        if (!reportsEnabled()) {
            return;
        }
        EntityMaid maid = event.getMaid();
        if (!isMonitored(maid) || MaidCondition.isUnconscious(maid)) {
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
        Origin origin = fromOwner ? new Origin("owner", "", "") : sourceOf(picked, maid);
        frame.addProperty("source", origin.kind());
        frame.addProperty("source_id", origin.id());
        frame.addProperty("source_name", origin.name());
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
     * 那份，普通食物那里是空的，认不出吃的是什么。玩家喂食也走 {@code startUsingItem} 了，
     * 所以要先问一句是不是刚喂的那口——喂的那一路已经由 {@link #onFed} 记过账。
     */
    public static void onItemUseFinish(LivingEntityUseItemEvent.Finish event) {
        if (!(event.getEntity() instanceof EntityMaid maid) || maid.level().isClientSide()) {
            return;
        }
        if (MaidFeeder.atePlayerFood(maid)) {
            return;
        }
        if (!reportsEnabled() || !isMonitored(maid) || MaidCondition.isUnconscious(maid)) {
            return;
        }
        ItemStack used = event.getItem();
        if (used.isEmpty() || used.getFoodProperties(maid) == null) {
            return;
        }
        selfFeeds.merge(itemId(used), 1, Integer::sum);
        selfFeedMaid = maid.getStringUUID();
        selfFeedCooldown = MEAL_DEBOUNCE_TICKS;
    }

    /** 每个服务端 tick 调一次：去抖到点就把攒下的推出去。 */
    public static void tick(BridgeServer bridge) {
        if (feedCooldown > 0 && --feedCooldown == 0) {
            pushItems(bridge, "item_fed", feedMaid, feeds);
            feeds.clear();
            feedMaid = "";
        }
        if (selfFeedCooldown > 0 && --selfFeedCooldown == 0) {
            pushItems(bridge, "maid_ate", selfFeedMaid, selfFeeds);
            selfFeeds.clear();
            selfFeedMaid = "";
        }
    }

    /**
     * 预先记一笔「这件东西本来就该出现在她身上」，背包差分按这个扣账。
     *
     * <p>玩家喂的那口会先摆在她手上，等她吃完才记 {@code item_fed}；这段时间里差分
     * 不能把它当成主人往背包里塞了同样的一件。
     */
    public static void suppressHandGain(ItemStack stack) {
        if (stack.isEmpty()) {
            return;
        }
        recentPickups.merge(itemId(stack), stack.getCount(), Integer::sum);
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
        sources.clear();
        feeds.clear();
        feedMaid = "";
        feedCooldown = 0;
        selfFeeds.clear();
        selfFeedMaid = "";
        selfFeedCooldown = 0;
        recentPickups.clear();
        pendingItem = null;
        pendingFromOwner = false;
    }

    /** 记一次「附近的死亡/方块破坏」。离她太远的不用记：她不会跑过去捡。 */
    private static void record(Level level, Vec3 pos, String kind, String id) {
        MinecraftServer server = level.getServer();
        if (server == null) {
            return;
        }
        EntityMaid maid = monitoredMaid(server);
        if (maid == null || maid.level() != level || MaidCondition.isUnconscious(maid)) {
            return;
        }
        double reach = MaidBridgeConfig.NOTICE_RADIUS.get() + 8.0;
        if (maid.position().distanceToSqr(pos) > reach * reach) {
            return;
        }
        long tick = server.getTickCount();
        sources.addLast(new Source(level, pos, kind, id, tick));
        while (sources.size() > SOURCE_LIMIT) {
            sources.removeFirst();
        }
    }

    /**
     * 这件东西是从哪来的：玩家丢的优先（拾取时物品还记着 thrower），其次找最近的
     * 死亡或方块破坏，超出时限或距离就算认不出来。
     */
    private static Origin sourceOf(ItemEntity item, EntityMaid maid) {
        if (item == null) {
            return Origin.UNKNOWN;
        }
        Entity thrower = item.getOwner();
        if (thrower != null && thrower != maid.getOwner()) {
            // 只有玩家才叫得出名字；别的实体（某些模组的发射器之类）只当认不出来
            return thrower instanceof ServerPlayer player
                    ? new Origin("player", "", player.getName().getString())
                    : Origin.UNKNOWN;
        }
        if (item.level().getServer() == null) {
            return Origin.UNKNOWN;
        }
        long now = item.level().getServer().getTickCount();
        double best = SOURCE_RADIUS * SOURCE_RADIUS;
        Source found = null;
        for (Source one : sources) {
            if (one.level() != item.level() || now - one.tick() > SOURCE_MEMORY_TICKS) {
                continue;
            }
            double distance = one.pos().distanceToSqr(item.position());
            if (distance <= best) {
                best = distance;
                found = one;
            }
        }
        return found == null ? Origin.UNKNOWN : new Origin(found.kind(), found.id(), "");
    }

    private static void pushItems(BridgeServer bridge, String type, String maidId, Map<String, Integer> items) {
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
