package com.lingchat.maidbridge.maid;

import com.lingchat.maidbridge.MaidBridgeConfig;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.food.FoodProperties;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;

/**
 * 她最喜欢吃的东西：配置里那几件物品。
 *
 * <p>配置只写物品 id，命名的前缀可以省（{@code cake} 就是 {@code minecraft:cake}）。
 * 主人把它拿在主手上时，车万女仆自己的讨食任务就会像见到蛋糕一样跑起来，见
 * {@code EntityMaidTemptationMixin}。
 *
 * <p>带负面效果的食物不算：毒马铃薯、河豚这类填进来也当没填——讨来是要吃的，
 * 不能让一口吃的把自己毒着。菜名解析失败、注册表里没有的 id 同样跳过。
 */
public final class FavoriteFood {
    private static List<? extends String> cachedRaw;
    private static Set<Item> cachedItems = Set.of();

    private FavoriteFood() {
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
        List<? extends String> raw = MaidBridgeConfig.FAVORITE_FOODS.get();
        if (raw.equals(cachedRaw)) {
            return cachedItems;
        }
        Set<Item> items = new LinkedHashSet<>();
        for (String one : raw) {
            Item item = resolve(one);
            if (item != null) {
                items.add(item);
            }
        }
        cachedRaw = raw;
        cachedItems = items;
        return items;
    }

    /** 配置里的一行 → 物品；不是食物、或带负面效果的给 null。 */
    private static Item resolve(String raw) {
        String id = raw == null ? "" : raw.trim();
        if (id.isEmpty()) {
            return null;
        }
        ResourceLocation key = ResourceLocation.tryParse(id.contains(":") ? id : "minecraft:" + id);
        Item item = key == null ? null : BuiltInRegistries.ITEM.getOptional(key).orElse(null);
        return item != null && edible(item) ? item : null;
    }

    /** 能吃，而且吃完不会中毒、反胃或挨饿。 */
    private static boolean edible(Item item) {
        FoodProperties food = item.getDefaultInstance().get(DataComponents.FOOD);
        if (food == null) {
            return false;
        }
        for (FoodProperties.PossibleEffect effect : food.effects()) {
            if (!effect.effect().getEffect().value().isBeneficial()) {
                return false;
            }
        }
        return true;
    }
}
