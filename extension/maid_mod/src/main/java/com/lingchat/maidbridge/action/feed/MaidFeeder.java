package com.lingchat.maidbridge.action.feed;

import com.github.tartaricacid.touhoulittlemaid.api.event.InteractMaidEvent;
import com.github.tartaricacid.touhoulittlemaid.api.event.MaidAfterEatEvent;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.github.tartaricacid.touhoulittlemaid.network.NetworkHandler;
import com.github.tartaricacid.touhoulittlemaid.network.message.SpawnParticlePackage;
import com.lingchat.maidbridge.integration.carry.CarryOnBridge;
import com.lingchat.maidbridge.integration.carry.CarryOnPresent;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.item.ItemReporter;
import java.util.HashMap;
import java.util.Iterator;
import java.util.Map;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.food.FoodProperties;
import net.minecraft.world.food.Foods;
import net.minecraft.world.item.ItemStack;

/**
 * 潜行 + 右键手里的食物 = 喂她一口。
 *
 * <p>一口不是一瞬：食物先放进她空着的那只手，她照车万女仆自己吃饭的方式
 * {@code startUsingItem}，模型把手里那件东西送向嘴边，吃完才真的消耗，然后冒一串爱心。
 * 车万女仆自己只认潜行喂金苹果（{@code ApplyGoldenAppleEvent}），普通食物右击只会打开
 * 她的界面，这里补的正是那一半；金苹果那一路仍由她自己处理（连成就都照旧），这里只负责
 * 事后补记。
 *
 * <p>拦截必须分两侧做：车万女仆的切坐姿（{@code SwitchSittingEvent}，LOWEST）没有判侧别，
 * 而右键实体时客户端也会本地跑一遍 {@code mobInteract}。只在服务端取消，客户端照样会把
 * 她的坐姿标记翻过去——模型坐下、服务端却还当她站着。所以这里挂在 HIGHEST 且两侧都取消；
 * 只有服务端才真的去喂、去记账。
 *
 * <p>空手、以及玩家手里正抱着东西时一律不碰：这两个手势在 Carry On 那边是抱起/放下，
 * 抢着取消会让交互卡在「她还在手里」的状态。
 */
public final class MaidFeeder {
    /** 金苹果交给车万女仆自己吃之后，等它真的吃完的时限（tick）。 */
    private static final int GOLDEN_TIMEOUT = 20;
    /** 一口吃完之后再宽限这么久，还没等到吃完事件就当没吃成（tick）。 */
    private static final int BITE_GRACE = 40;

    /** 已经递到嘴边、还没等到「吃完」的那一口。 */
    private static final class Bite {
        final ItemStack food;
        final int nutrition;
        /** 是我们塞到她手上的（要还手、要回血），还是只等车万女仆自己吃完补记。 */
        final boolean ours;
        int ticks;

        Bite(ItemStack food, int nutrition, int ticks, boolean ours) {
            this.food = food;
            this.nutrition = nutrition;
            this.ticks = ticks;
            this.ours = ours;
        }
    }

    private static final Map<String, Bite> BITES = new HashMap<>();
    /** 刚吃完玩家喂的那一口：同一 tick 里她自己的进食记账要跳过，别报成她自己找食吃。 */
    private static String justFed = "";

    private MaidFeeder() {
    }

    /** 交互的最前面：两侧都拦下这个手势，服务端再把这一口真的喂出去。 */
    public static void onCapture(InteractMaidEvent event) {
        if (!isFeedGesture(event)) {
            return;
        }
        // 两侧都要取消：车万女仆的切坐姿不判侧别，客户端不拦就会只把模型翻成坐姿
        event.setCanceled(true);
        if (event.getWorld().isClientSide()) {
            return;
        }
        EntityMaid maid = event.getMaid();
        Player player = event.getPlayer();
        ItemStack stack = event.getStack();
        FoodProperties food = stack.getFoodProperties(maid);
        String maidId = maid.getStringUUID();
        if (food == Foods.GOLDEN_APPLE || food == Foods.ENCHANTED_GOLDEN_APPLE) {
            // 金苹果由车万女仆在 NORMAL 处理，这里只留副本，不抢
            BITES.put(maidId, new Bite(stack.copy(), food.nutrition(), GOLDEN_TIMEOUT, false));
            return;
        }
        // 她正举着盾或已经在吃别的东西：这一口先喂不进去，手势只当没发生
        if (maid.isUsingItem()) {
            return;
        }
        InteractionHand hand = emptyHand(maid);
        if (hand == null) {
            // 两只手都占着：退回当场吃掉，不为一口饭把她手里的活计挪走
            eatNow(maid, player, stack, food);
            return;
        }
        ItemStack bite = stack.copy();
        bite.setCount(1);
        // 创造的玩家不该真的少东西：交互成功时 Player.interactOn 会把计数补回来
        stack.shrink(1);
        maid.setItemInHand(hand, bite);
        maid.startUsingItem(hand);
        // 这一件是先摆在她手上、吃完才记账的：背包差分别把它当成「刚收到」
        ItemReporter.suppressHandGain(bite);
        BITES.put(maidId, new Bite(bite.copy(), food.nutrition(),
                bite.getUseDuration(maid) + BITE_GRACE, true));
    }

    /**
     * 她真的吃完了：记账、回血、冒爱心。
     *
     * <p>金苹果那一路也在这里收口。事件被别的模组取消时等不到它，那一口会在
     * {@link #tick} 里作废。
     */
    public static void onAfterEat(MaidAfterEatEvent event) {
        EntityMaid maid = event.getMaid();
        if (maid.level().isClientSide()) {
            return;
        }
        Bite bite = BITES.remove(maid.getStringUUID());
        if (bite == null) {
            return;
        }
        justFed = maid.getStringUUID();
        ItemReporter.onFed(maid, bite.food);
        if (bite.ours) {
            maid.heal(Math.max(1, bite.nutrition));
        }
        heart(maid);
    }

    /** 这一 tick 玩家喂的那口刚吃完：她自己进食的记账别把同一件事再说一遍。 */
    public static boolean atePlayerFood(EntityMaid maid) {
        return maid != null && maid.getStringUUID().equals(justFed);
    }

    /** 每个服务端 tick 调一次：没吃成的那口把食物还给她，别留在她手里。 */
    public static void tick(MinecraftServer server) {
        justFed = "";
        if (BITES.isEmpty()) {
            return;
        }
        for (Iterator<Map.Entry<String, Bite>> it = BITES.entrySet().iterator(); it.hasNext(); ) {
            Map.Entry<String, Bite> entry = it.next();
            Bite bite = entry.getValue();
            if (!bite.ours) {
                if (--bite.ticks <= 0) {
                    it.remove();
                }
                continue;
            }
            EntityMaid maid = MaidQueries.findMaidById(server, entry.getKey());
            // 换了她手上的东西，或者她已经停下：这一口没吃成
            if (maid != null && (!maid.isUsingItem() || !maid.getUseItem().is(bite.food.getItem()))) {
                returnBite(maid, bite);
                it.remove();
                continue;
            }
            if (--bite.ticks <= 0) {
                if (maid != null) {
                    returnBite(maid, bite);
                }
                it.remove();
            }
        }
    }

    /** 停服、换存档时清干净。 */
    public static void clear() {
        BITES.clear();
        justFed = "";
    }

    private static boolean isFeedGesture(InteractMaidEvent event) {
        EntityMaid maid = event.getMaid();
        Player player = event.getPlayer();
        if (maid.getOwner() != player || !player.isShiftKeyDown()
                || maid.isSleeping() || maid.isPassenger()) {
            return false;
        }
        // 玩家手里抱着东西时是放下/抱起的手势，交给 Carry On 自己处理
        if (CarryOnPresent.LOADED && CarryOnBridge.isCarrying(player)) {
            return false;
        }
        ItemStack stack = event.getStack();
        return !stack.isEmpty() && stack.getFoodProperties(maid) != null;
    }

    /** 挑一只空着的手：正手优先，两只都占着就给 null。 */
    private static InteractionHand emptyHand(EntityMaid maid) {
        for (InteractionHand hand : InteractionHand.values()) {
            if (maid.getItemInHand(hand).isEmpty()) {
                return hand;
            }
        }
        return null;
    }

    /** 两只手都没空时的老做法：当场吃掉，不出动画。 */
    private static void eatNow(EntityMaid maid, Player player, ItemStack stack, FoodProperties food) {
        ItemReporter.onFed(maid, stack.copy());
        maid.eat(maid.level(), player.isCreative() ? stack.copy() : stack);
        maid.heal(Math.max(1, food.nutrition()));
        heart(maid);
    }

    /** 原版女仆吃完东西的样子：一小串爱心。 */
    private static void heart(EntityMaid maid) {
        NetworkHandler.sendToNearby(maid, new SpawnParticlePackage(maid.getId(), SpawnParticlePackage.Type.HEART));
    }

    /** 没吃成：这一口掉回她脚下，别留在她手里。 */
    private static void returnBite(EntityMaid maid, Bite bite) {
        ItemStack hand = maid.getItemInHand(InteractionHand.OFF_HAND);
        ItemStack main = maid.getItemInHand(InteractionHand.MAIN_HAND);
        // 食物在哪只手就收回哪只：喂的时候是塞进空手里的，不会两边都有
        if (!hand.isEmpty() && hand.is(bite.food.getItem())) {
            maid.setItemInHand(InteractionHand.OFF_HAND, ItemStack.EMPTY);
            ItemReporter.markRemoval(hand);
            maid.spawnAtLocation(hand);
        } else if (!main.isEmpty() && main.is(bite.food.getItem())) {
            maid.setItemInHand(InteractionHand.MAIN_HAND, ItemStack.EMPTY);
            ItemReporter.markRemoval(main);
            maid.spawnAtLocation(main);
        }
    }
}
