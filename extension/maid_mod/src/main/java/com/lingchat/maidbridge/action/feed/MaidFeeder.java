package com.lingchat.maidbridge.action.feed;

import com.github.tartaricacid.touhoulittlemaid.api.event.InteractMaidEvent;
import com.github.tartaricacid.touhoulittlemaid.api.event.MaidAfterEatEvent;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.integration.carry.CarryOnBridge;
import com.lingchat.maidbridge.integration.carry.CarryOnPresent;
import com.lingchat.maidbridge.perception.item.ItemReporter;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.food.FoodProperties;
import net.minecraft.world.food.Foods;
import net.minecraft.world.item.ItemStack;

/**
 * 潜行 + 右键手里的食物 = 喂她一口。
 *
 * <p>车万女仆自己只认潜行喂金苹果（{@code ApplyGoldenAppleEvent}），普通食物右击只会
 * 打开她的界面。这里补的正是那一半：吃一个、按营养值回一点血、出进食音效，再把「吃了
 * 什么」当事件交出去。金苹果那一路仍由她自己处理（连成就都照旧），这里只负责事后说一声。
 *
 * <p>挂载方式：普通食物在 {@code HIGHEST} 当场吃掉并取消事件——车万女仆的切坐姿在
 * {@code LOWEST}，别的附属模组还可能更早接管交互，只有抢在最前面，潜行喂饭才不会被
 * 顺带坐一下或者被别的右键逻辑截走。金苹果由她自己吃，这里在 {@code HIGHEST} 留一份
 * 物品副本，等她真的吃完（{@code MaidAfterEatEvent}）再补记，不重复吃、也不误报。
 *
 * <p>空手与被抱着时一律不碰：这两个手势在 Carry On 那边是抱起/放下，抢着取消会让交互
 * 卡在「她还在手里」的状态。
 */
public final class MaidFeeder {
    /** 金苹果在 HIGHEST 记下、真正吃完时消费：哪只女仆、吃掉前那份物品的副本。 */
    private static String goldenMaid = "";
    private static ItemStack goldenStack = ItemStack.EMPTY;
    /** 交互没走到「吃」就作废，别把副本留到下一次喂食。 */
    private static int goldenTicks;

    private MaidFeeder() {
    }

    /** 交互的最前面：普通食物当场喂掉；金苹果先留一份副本，交给车万女仆自己吃。 */
    public static void onCapture(InteractMaidEvent event) {
        resetGolden();
        if (event.getWorld().isClientSide()) {
            return;
        }
        Player player = event.getPlayer();
        EntityMaid maid = event.getMaid();
        if (maid.getOwner() != player || !player.isShiftKeyDown() || maid.isSleeping()) {
            return;
        }
        // 空手不喂；玩家手里抱着东西（Carry On）或被抱起来（成为乘客）也不碰，
        // 那边是抱起/放下，抢着取消会让交互卡在「她还在手里」
        if (CarryOnPresent.LOADED && CarryOnBridge.isCarrying(player)) {
            return;
        }
        ItemStack stack = event.getStack();
        if (stack.isEmpty() || maid.isPassenger()) {
            return;
        }
        FoodProperties food = stack.getFoodProperties(maid);
        if (food == null) {
            return;
        }
        if (food == Foods.GOLDEN_APPLE || food == Foods.ENCHANTED_GOLDEN_APPLE) {
            // 金苹果由车万女仆在 NORMAL 处理，这里只留副本，不抢
            goldenMaid = maid.getStringUUID();
            goldenStack = stack.copy();
            goldenTicks = 20;
            return;
        }
        // 抢在所有交互之前取消：切坐姿（LOWEST）与别的附属模组的右键逻辑都收不到
        // 这个事件，潜行拿着食物出手就只剩「喂她」这一种结果。
        ItemReporter.onFed(maid, stack.copy());
        // 创造的玩家不该真的少东西，但音效、效果与「吃了」这件事照旧
        maid.eat(maid.level(), player.isCreative() ? stack.copy() : stack);
        maid.heal(Math.max(1, food.nutrition()));
        event.setCanceled(true);
    }

    /** 她真的吃完了：金苹果那一路在这里补记；事件被别的模组取消时就不会误报。 */
    public static void onAfterEat(MaidAfterEatEvent event) {
        EntityMaid maid = event.getMaid();
        if (maid.level().isClientSide() || goldenStack.isEmpty()
                || !goldenMaid.equals(maid.getStringUUID())) {
            return;
        }
        ItemStack eaten = goldenStack;
        resetGolden();
        ItemReporter.onFed(maid, eaten);
    }

    /** 每个服务端 tick 调一次：金苹果的交互没走到吃就作废。 */
    public static void tick() {
        if (goldenStack.isEmpty()) {
            return;
        }
        if (--goldenTicks <= 0) {
            resetGolden();
        }
    }

    /** 停服、换存档时清干净。 */
    public static void clear() {
        resetGolden();
    }

    private static void resetGolden() {
        goldenMaid = "";
        goldenStack = ItemStack.EMPTY;
        goldenTicks = 0;
    }
}
