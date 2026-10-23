package com.lingchat.maidbridge.maid;

import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.food.FoodProperties;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;

/**
 * 她最喜欢吃的东西，由 LingChat 那一侧发过来。
 *
 * <p>这是角色的设定，跟着插件里的配置走，所以模组这边不设配置项：连上时与玩家每次
 * 开口前各收一份（见 {@code set_favorite_food}），只存着照用。
 *
 * <p>填法：写完整的物品 id 就按那个命名空间认；只写后半段（{@code cooked_rice}）时
 * 原版优先，其余模组里同名的也算上。带负面效果的食物（毒马铃薯、河豚这类）一律不算——
 * 讨来是要吃的，不能让一口吃的把自己毒着。认不出是不是食物的（有些模组食物不带原版
 * 的食物组件）照常算数，不替玩家拒绝。
 */
public final class FavoriteFood {
    private static List<? extends String> received = List.of();
    private static List<? extends String> cachedRaw;
    private static Set<Item> cachedItems = Set.of();

    private FavoriteFood() {
    }

    /** 收到一份新的清单。 */
    public static void set(List<? extends String> items) {
        received = List.copyOf(items);
    }

    /** 停服、换存档时清掉：下一次连上会重新发一遍。 */
    public static void clear() {
        received = List.of();
    }

    /** 这一件是不是她最爱吃的。 */
    public static boolean holding(ItemStack stack) {
        return !stack.isEmpty() && items().contains(stack.getItem());
    }

    /** 物品的注册表 id，事件里用这个。 */
    public static String id(ItemStack stack) {
        return BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
    }

    private static Set<Item> items() {
        if (received.equals(cachedRaw)) {
            return cachedItems;
        }
        Set<Item> items = new LinkedHashSet<>();
        for (String one : received) {
            resolve(one, items);
        }
        cachedRaw = received;
        cachedItems = items;
        return items;
    }

    /** 把配置里的一行加进清单：写了前缀按前缀认，没写就原版优先、再按路径找同名。 */
    private static void resolve(String raw, Set<Item> out) {
        String id = raw == null ? "" : raw.trim().toLowerCase(Locale.ROOT);
        if (id.isEmpty()) {
            return;
        }
        if (id.contains(":")) {
            add(out, BuiltInRegistries.ITEM.getOptional(ResourceLocation.tryParse(id)).orElse(null));
            return;
        }
        add(out, BuiltInRegistries.ITEM.getOptional(ResourceLocation.tryParse("minecraft:" + id)).orElse(null));
        for (Item item : BuiltInRegistries.ITEM) {
            if (BuiltInRegistries.ITEM.getKey(item).getPath().equals(id)) {
                add(out, item);
            }
        }
    }

    private static void add(Set<Item> out, Item item) {
        if (item != null && safe(item)) {
            out.add(item);
        }
    }

    /** 吃完不会中毒、反胃或挨饿。认不出食物组件的当作可以讨。 */
    private static boolean safe(Item item) {
        FoodProperties food = item.getDefaultInstance().get(DataComponents.FOOD);
        if (food == null) {
            return true;
        }
        for (FoodProperties.PossibleEffect effect : food.effects()) {
            if (!effect.effect().getEffect().value().isBeneficial()) {
                return false;
            }
        }
        return true;
    }
}
