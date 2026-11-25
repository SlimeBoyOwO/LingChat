package com.lingchat.maidbridge.debug;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.construction.MaidBuildController;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.suggestion.SuggestionProvider;
import java.util.EnumSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.Commands;
import net.minecraft.network.chat.Component;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;
import net.neoforged.neoforge.event.RegisterCommandsEvent;

/**
 * 调试指令：/tlml pathing show|hide|status。
 *
 * <p>show 盯住离执行者最近的那只女仆（64 格内），可以只显示其中几部分；不跟名字时
 * 全部开 / 全部关二选一。她开始施工之后，画出的内容与 {@link PathDebug.Layer} 一一对应。
 * hide 不带名字是整体关掉，带名字只关那几部分。status 打印当前阶段与开着哪些部分。
 * 只服务端画原版粒子，不需要客户端模组。
 */
public final class PathingCommands {
    /** 找女仆的范围：再远就不知道说的是哪一只，也不想在全图里逐个找。 */
    private static final double SEARCH_RADIUS = 64.0;
    private static final String LAYERS = "layers";

    /** 补全当前这个词，已经写过的同名部分不重复建议。 */
    private static final SuggestionProvider<CommandSourceStack> LAYER_SUGGESTIONS =
            (context, builder) -> {
                String remaining = builder.getRemaining();
                int cut = remaining.lastIndexOf(' ') + 1;
                String prefix = remaining.substring(cut).toLowerCase(Locale.ROOT);
                var offset = builder.createOffset(builder.getStart() + cut);
                for (PathDebug.Layer layer : PathDebug.Layer.values()) {
                    if (layer.id().startsWith(prefix)) {
                        offset.suggest(layer.id());
                    }
                }
                return offset.buildFuture();
            };

    private PathingCommands() {
    }

    public static void register(RegisterCommandsEvent event) {
        event.getDispatcher().register(Commands.literal("tlml")
                .then(Commands.literal("pathing")
                        // 只是画粒子，但调试指令按惯例只给管理员（单人游戏要开作弊）
                        .requires(source -> source.hasPermission(2))
                        .then(Commands.literal("show")
                                .executes(context -> show(context.getSource(), ""))
                                .then(Commands.argument(LAYERS, StringArgumentType.greedyString())
                                        .suggests(LAYER_SUGGESTIONS)
                                        .executes(context -> show(
                                                context.getSource(),
                                                StringArgumentType.getString(context, LAYERS)))))
                        .then(Commands.literal("hide")
                                .executes(context -> hideAll(context.getSource()))
                                .then(Commands.argument(LAYERS, StringArgumentType.greedyString())
                                        .suggests(LAYER_SUGGESTIONS)
                                        .executes(context -> hide(
                                                context.getSource(),
                                                StringArgumentType.getString(context, LAYERS)))))
                        .then(Commands.literal("status")
                                .executes(context -> status(context.getSource())))));
    }

    private static int show(CommandSourceStack source, String raw) {
        Set<PathDebug.Layer> selected = EnumSet.noneOf(PathDebug.Layer.class);
        String unknown = parseLayers(raw, selected);
        if (unknown != null) {
            source.sendFailure(Component.literal(
                    "没有这个显示：" + unknown + "。可选：" + PathDebug.layerNames()));
            return 0;
        }
        EntityMaid maid = nearest(source);
        if (maid == null) {
            source.sendFailure(Component.literal("她不在附近（要 64 格内，且区块已加载）"));
            return 0;
        }
        PathDebug.show(maid, selected);
        source.sendSuccess(() -> Component.literal(
                "开始盯着 " + maid.getName().getString()
                        + "；现在显示：" + PathDebug.layersText()), false);
        return 1;
    }

    private static int hideAll(CommandSourceStack source) {
        PathDebug.stop();
        source.sendSuccess(() -> Component.literal("寻路可视化已关闭"), false);
        return 1;
    }

    private static int hide(CommandSourceStack source, String raw) {
        Set<PathDebug.Layer> selected = EnumSet.noneOf(PathDebug.Layer.class);
        String unknown = parseLayers(raw, selected);
        if (unknown != null) {
            source.sendFailure(Component.literal(
                    "没有这个显示：" + unknown + "。可选：" + PathDebug.layerNames()));
            return 0;
        }
        PathDebug.hideLayers(selected);
        source.sendSuccess(
                () -> Component.literal("现在显示：" + PathDebug.layersText()), false);
        return 1;
    }

    private static int status(CommandSourceStack source) {
        String text = PathDebug.status() + MaidBuildController.debugStatus();
        source.sendSuccess(() -> Component.literal(text), false);
        return 1;
    }

    /** 把空白或逗号隔开的名字收进 out；有不认识的名字就原样返回它。 */
    private static String parseLayers(String raw, Set<PathDebug.Layer> out) {
        for (String token : raw.trim().split("[\\s,]+")) {
            if (token.isEmpty()) {
                continue;
            }
            PathDebug.Layer layer = PathDebug.Layer.byName(token.toLowerCase(Locale.ROOT));
            if (layer == null) {
                return token;
            }
            out.add(layer);
        }
        return null;
    }

    /** 离执行者最近的一只活着的女仆。 */
    private static EntityMaid nearest(CommandSourceStack source) {
        ServerLevel level = source.getLevel();
        Vec3 at = source.getPosition();
        List<EntityMaid> maids = level.getEntitiesOfClass(
                EntityMaid.class, new AABB(at, at).inflate(SEARCH_RADIUS), Entity::isAlive);
        EntityMaid best = null;
        double bestDistance = Double.MAX_VALUE;
        for (EntityMaid maid : maids) {
            double distance = maid.distanceToSqr(at);
            if (distance < bestDistance) {
                bestDistance = distance;
                best = maid;
            }
        }
        return best;
    }
}
