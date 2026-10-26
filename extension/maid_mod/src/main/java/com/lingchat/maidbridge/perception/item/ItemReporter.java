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
import java.util.UUID;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.food.FoodProperties;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.Level;
import net.minecraft.world.phys.Vec3;
import net.neoforged.neoforge.event.entity.item.ItemTossEvent;
import net.neoforged.neoforge.event.entity.living.LivingDropsEvent;
import net.neoforged.neoforge.event.entity.living.LivingEntityUseItemEvent;
import net.neoforged.neoforge.event.entity.living.LivingDeathEvent;
import net.neoforged.neoforge.event.level.BlockDropsEvent;
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
 * <p>玩家在女仆界面里把东西放进背包、或从背包里拿走，都没有事件可挂，那两路由
 * {@link BackpackWatcher} 每轮采样比出来；它会把这里记下的账扣掉——捡起来的算「多出来
 * 的那份」，她自己吃掉与交出去的算「少掉的那份」，同一件事只说一遍。
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
    /**
     * 她自己破坏方块之后的记性（tick）。车万女仆的干活（农场、甘蔗、清雪……）与我们的挖矿
     * 都把掉落直接塞进她的背包，不走拾取事件；这之后的一小段时间里背包增量都是她自己挣的。
     */
    private static final int SELF_GATHER_TICKS = 120;

    /** 最近一次死亡或方块被破坏的位置：用来认地上那件东西是哪来的。 */
    private record Source(Level level, Vec3 pos, String kind, String id, long tick) {
    }

    /**
     * 这件东西的来源：谁丢的、哪只生物掉的、哪种方块崩出来的。
     *
     * <p>{@code uuid} 只在玩家丢出来的那一路有值：凭它认「是主人丢的」还是「别人丢的」，
     * 不再看主人站得多近。
     */
    private record Origin(String kind, String id, String name, String uuid) {
        static final Origin UNKNOWN = new Origin("unknown", "", "", "");
    }

    /**
     * 掉落物身上记来源的 NBT 键名。
     *
     * <p>原版只有玩家丢出的物品记着 thrower，挖方块崩出来的、生物掉的都不记；主人在旁边
     * 时原来会被一律说成「主人丢下的」。所以掉落一出生就在实体上盖一个来源戳，她捡起来时
     * 照着念。戳跟着实体走，存盘读档也不会丢。
     */
    private static final String SOURCE_TAG = "LingChatSource";

    private static final Deque<Source> sources = new ArrayDeque<>();
    private static final Map<String, Integer> feeds = new LinkedHashMap<>();
    /** 吃完剩下的容器（蘑菇煲的碗这类），和这一顿一起报。 */
    private static final Map<String, Integer> feedLeftovers = new LinkedHashMap<>();
    private static String feedMaid = "";
    private static int feedCooldown;
    /** 她自己从背包里找东西吃（车万女仆的进食系统），与玩家喂食分开说。 */
    private static final Map<String, Integer> selfFeeds = new LinkedHashMap<>();
    private static final Map<String, Integer> selfFeedLeftovers = new LinkedHashMap<>();
    private static String selfFeedMaid = "";
    private static int selfFeedCooldown;
    /** 上一次采样之后她捡起来的物品：背包差分要把这部分扣掉。 */
    private static final Map<String, Integer> recentPickups = new LinkedHashMap<>();
    /**
     * 她自己弄没的东西：吃掉的那口、交给主人的那几件、没吃成掉回地上的。
     *
     * <p>背包差分把这份扣掉，否则主人站在旁边时，这些都会变成「主人从背包里拿走了」。
     */
    private static final Map<String, Integer> recentRemovals = new LinkedHashMap<>();
    /** 这一次捡的东西是哪来的（Pre 与 Post 在同一次拾取调用里前后脚触发）。 */
    private static ItemEntity pendingItem;
    private static Origin pendingOrigin;
    private static boolean pendingFromOwner;
    /** 她自己破坏方块的最近一次：掉落被车万女仆直接塞进背包，背包增量要算成她自己的。 */
    private static String selfGatherMaid = "";
    private static long selfGatherTick;

    private ItemReporter() {
    }

    /** 她自己破坏了一块方块（掉落直接进背包）：这之后的背包增量不算「谁给她的」。 */
    public static void markSelfGather(EntityMaid maid) {
        if (maid == null) {
            return;
        }
        selfGatherMaid = maid.getStringUUID();
        selfGatherTick = maid.level().getGameTime();
    }

    /**
     * 她刚刚自己破坏过方块：背包差分不该把这些增量说成「玩家放进来的」。
     *
     * <p>车万女仆干活的掉落直接进背包（不走拾取事件），主人在旁边时原来每一份收成都被
     * 说成「主人往背包里放了小麦」。与挖矿那条是同一类：那段时间的增量归她自己，不需要
     * 另报一条。
     */
    public static boolean gatheredRecently(EntityMaid maid) {
        if (maid == null || selfGatherMaid.isEmpty() || !selfGatherMaid.equals(maid.getStringUUID())) {
            return false;
        }
        return maid.level().getGameTime() - selfGatherTick <= SELF_GATHER_TICKS;
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

    /** 玩家丢出来的东西（Q 或从界面里拖出去）：盖章，说成「谁丢下的」。 */
    public static void onToss(ItemTossEvent event) {
        Player player = event.getPlayer();
        if (player.level().isClientSide()) {
            return;
        }
        tag(event.getEntity(), "player", "", player.getName().getString(), player.getUUID());
    }

    /** 方块掉落：盖章说成「挖哪种方块崩出来的」，不管主人是不是就站在旁边。 */
    public static void onBlockDrops(BlockDropsEvent event) {
        String block = BuiltInRegistries.BLOCK.getKey(event.getState().getBlock()).toString();
        for (ItemEntity item : event.getDrops()) {
            tag(item, "block", block, "", null);
        }
    }

    /** 生物掉落：盖章说成「哪只生物掉的」；玩家死了掉出来的按玩家丢的说。 */
    public static void onLivingDrops(LivingDropsEvent event) {
        LivingEntity dead = event.getEntity();
        if (dead.level().isClientSide()) {
            return;
        }
        if (dead instanceof Player player) {
            for (ItemEntity item : event.getDrops()) {
                tag(item, "player", "", player.getName().getString(), player.getUUID());
            }
            return;
        }
        String type = SurroundingsReader.typeId(dead);
        for (ItemEntity item : event.getDrops()) {
            tag(item, "mob", type, "", null);
        }
    }

    /** 拾取前的判定：这件东西是谁丢的。模拟拾取（只探能不能装下）不算。 */
    public static void onPickupPre(MaidPickupEvent.ItemResultPre event) {
        pendingFromOwner = false;
        pendingItem = null;
        pendingOrigin = null;
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
        pendingOrigin = tagged(item);
        if (pendingOrigin != null) {
            // 有来源戳就照着戳认：主人丢的、别人丢的、方块崩的、生物掉的各归各位
            pendingFromOwner = "player".equals(pendingOrigin.kind())
                    && owner != null && owner.getStringUUID().equals(pendingOrigin.uuid());
        } else {
            // 老存档里已经在地上的东西没有戳，退回原版 thrower
            pendingFromOwner = owner != null && item != null && item.getOwner() == owner;
        }
    }

    /** 她真的把东西收进背包了。别人的东西也报，只是「主人给的」才值得她当场开口（插件判）。 */
    public static void onPickupPost(MaidPickupEvent.ItemResultPost event) {
        boolean fromOwner = pendingFromOwner;
        ItemEntity picked = pendingItem;
        Origin origin = pendingOrigin;
        pendingFromOwner = false;
        pendingItem = null;
        pendingOrigin = null;
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
        if (origin == null) {
            origin = sourceOf(picked, maid);
        }
        frame.addProperty("source", fromOwner ? "owner" : origin.kind());
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
        // 这一口先摆在她手上、吃完就没了：账上算她自己弄没的，别让背包差分说成被拿走了
        markRemoval(itemId(eaten), 1);
        mergeLeftover(feedLeftovers, leftoverOf(maid, eaten));
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
        // 她自己吃掉的这份：账上记一笔，背包差分不会说成「主人从背包里拿走了」
        markRemoval(itemId(used), 1);
        mergeLeftover(selfFeedLeftovers, leftoverOf(maid, used));
        selfFeedMaid = maid.getStringUUID();
        selfFeedCooldown = MEAL_DEBOUNCE_TICKS;
    }

    /** 每个服务端 tick 调一次：去抖到点就把攒下的推出去。 */
    public static void tick(BridgeServer bridge) {
        if (feedCooldown > 0 && --feedCooldown == 0) {
            pushItems(bridge, "item_fed", feedMaid, feeds, feedLeftovers);
            feeds.clear();
            feedLeftovers.clear();
            feedMaid = "";
        }
        if (selfFeedCooldown > 0 && --selfFeedCooldown == 0) {
            pushItems(bridge, "maid_ate", selfFeedMaid, selfFeeds, selfFeedLeftovers);
            selfFeeds.clear();
            selfFeedLeftovers.clear();
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

    /**
     * 记一笔「这件东西是她自己弄没的」：吃掉的那口、交给主人的那几件、没吃成掉回地上的。
     *
     * <p>与 {@link #suppressHandGain} 是同一本账的另一半：那一半防「多出来被当成收到」，
     * 这一半防「少下去被当成被拿走」。她正在干活时背包本来就会少东西（农场吃种子、弓吃箭），
     * 那种整段路不走差分，不必记在这里。
     */
    public static void markRemoval(ItemStack stack) {
        if (stack.isEmpty()) {
            return;
        }
        markRemoval(itemId(stack), stack.getCount());
    }

    public static void markRemoval(String item, int count) {
        if (item == null || item.isEmpty() || count <= 0) {
            return;
        }
        recentRemovals.merge(item, count, Integer::sum);
    }

    /** 取走并清空「上一次采样之后她自己弄没的物品」，供背包差分扣账。 */
    public static Map<String, Integer> drainRecentRemovals() {
        Map<String, Integer> copy = new LinkedHashMap<>(recentRemovals);
        recentRemovals.clear();
        return copy;
    }

    /** 停服、换存档、没人听的时候清干净：别把上一波东西算到下一次头上。 */
    public static void clear() {
        sources.clear();
        feeds.clear();
        feedMaid = "";
        feedCooldown = 0;
        selfFeeds.clear();
        feedLeftovers.clear();
        selfFeedLeftovers.clear();
        selfFeedMaid = "";
        selfFeedCooldown = 0;
        recentPickups.clear();
        recentRemovals.clear();
        pendingItem = null;
        pendingFromOwner = false;
        selfGatherMaid = "";
        selfGatherTick = 0;
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
                    ? new Origin("player", "", player.getName().getString(), player.getStringUUID())
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
        return found == null ? Origin.UNKNOWN : new Origin(found.kind(), found.id(), "", "");
    }

    /** 给这件掉落物盖一个来源戳（只在服务端做）。 */
    private static void tag(ItemEntity item, String kind, String id, String name, UUID uuid) {
        if (item == null || item.level().isClientSide()) {
            return;
        }
        CompoundTag tag = new CompoundTag();
        tag.putString("kind", kind);
        if (!id.isEmpty()) {
            tag.putString("id", id);
        }
        if (!name.isEmpty()) {
            tag.putString("name", name);
        }
        if (uuid != null) {
            tag.putUUID("uuid", uuid);
        }
        item.getPersistentData().put(SOURCE_TAG, tag);
    }

    /** 这件东西身上的来源戳；老存档里没盖过章的给 null。 */
    private static Origin tagged(ItemEntity item) {
        if (item == null) {
            return null;
        }
        CompoundTag data = item.getPersistentData();
        if (!data.contains(SOURCE_TAG, Tag.TAG_COMPOUND)) {
            return null;
        }
        CompoundTag tag = data.getCompound(SOURCE_TAG);
        String kind = tag.getString("kind");
        if (kind.isEmpty()) {
            return null;
        }
        return new Origin(
                kind,
                tag.getString("id"),
                tag.getString("name"),
                tag.contains("uuid") ? tag.getUUID("uuid").toString() : "");
    }

    private static void pushItems(
            BridgeServer bridge, String type, String maidId,
            Map<String, Integer> items, Map<String, Integer> leftovers) {
        if (items.isEmpty() || bridge == null || !bridge.hasClients()) {
            return;
        }
        JsonObject frame = EventFrames.base(type, maidId);
        frame.add("items", arrayOf(items));
        if (!leftovers.isEmpty()) {
            frame.add("leftover", arrayOf(leftovers));
        }
        bridge.broadcastEvent(frame);
    }

    private static JsonArray arrayOf(Map<String, Integer> items) {
        JsonArray array = new JsonArray();
        for (Map.Entry<String, Integer> entry : items.entrySet()) {
            JsonObject one = new JsonObject();
            one.addProperty("item", entry.getKey());
            one.addProperty("count", entry.getValue());
            array.add(one);
        }
        return array;
    }

    /**
     * 吃完剩下的容器（蘑菇煲、兔肉煲的碗这类）。
     *
     * <p>车万女仆自己会把 {@code usingConvertsTo} 塞进她的背包（装不下才掉在脚边），
     * 背包差分会把它当成「主人刚放进来的碗」——这里预先记进差分账，同时跟这一顿饭
     * 一起报给插件，她才不会莫名其妙多出一只碗。
     */
    private static void mergeLeftover(Map<String, Integer> into, ItemStack leftover) {
        if (leftover.isEmpty()) {
            return;
        }
        into.merge(itemId(leftover), leftover.getCount(), Integer::sum);
        suppressHandGain(leftover);
    }

    private static ItemStack leftoverOf(EntityMaid maid, ItemStack eaten) {
        FoodProperties food = eaten.getFoodProperties(maid);
        return food == null ? ItemStack.EMPTY : food.usingConvertsTo().orElse(ItemStack.EMPTY);
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
