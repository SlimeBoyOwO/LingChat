package com.lingchat.maidbridge.debug;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.action.construction.MaidBuildController;
import com.lingchat.maidbridge.action.core.MaidAction;
import com.lingchat.maidbridge.action.core.MaidActionController;
import com.lingchat.maidbridge.action.mine.MineRules;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.suggestion.SuggestionProvider;
import java.util.EnumSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.Commands;
import net.minecraft.core.BlockPos;
import net.minecraft.network.chat.Component;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.Vec3;
import net.neoforged.neoforge.event.RegisterCommandsEvent;

/**
 * 调试指令：/tlml pathing show|hide|status，以及 /tlml test goto|build。
 *
 * <p>show 盯住离执行者最近的那只女仆（64 格内），可以只显示其中几部分；不跟名字时
 * 全部开 / 全部关二选一。她开始施工之后，画出的内容与 {@link PathDebug.Layer} 一一对应。
 * hide 不带名字是整体关掉，带名字只关那几部分。status 打印当前阶段与开着哪些部分。
 * 只服务端画原版粒子，不需要客户端模组。进存档时可视化默认就在自动盯着最近的她。
 *
 * <p>test 是给寻路与施工做单测的入口：不经过 LLM 与插件，直接走「走到主人身边」和
 * 「按图纸施工」这两条工具路径，结束原因事后发到执行者的聊天栏。
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
                                .executes(context -> status(context.getSource()))))
                .then(Commands.literal("test")
                        .requires(source -> source.hasPermission(2))
                        .then(Commands.literal("goto")
                                .executes(context -> testGoto(context.getSource())))
                        .then(Commands.literal("build")
                                .executes(context -> testBuild(context.getSource(), true))
                                .then(Commands.literal("house")
                                        .executes(context -> testBuild(context.getSource(), true)))
                                .then(Commands.literal("climb")
                                        .executes(context -> testBuild(context.getSource(), false))))));
    }

    /** 直接走「走到主人身边」那条工具路径：目标就是执行者此刻站的格子。 */
    private static int testGoto(CommandSourceStack source) {
        ServerPlayer player = source.getPlayer();
        if (player == null) {
            source.sendFailure(Component.literal("这条要玩家自己执行：目标就是执行者站的格子"));
            return 0;
        }
        EntityMaid maid = nearest(source);
        if (maid == null) {
            source.sendFailure(Component.literal("她不在附近（要 64 格内，且区块已加载）"));
            return 0;
        }
        ServerPlayer owner = MaidQueries.owner(maid);
        if (owner == null || !owner.getUUID().equals(player.getUUID())) {
            source.sendFailure(Component.literal("她不是你的女仆：走过来测的是主人所在的那一格"));
            return 0;
        }
        PathDebug.show(maid, EnumSet.allOf(PathDebug.Layer.class));
        JsonObject frame = MaidActionController.start(
                source.getServer(), maid,
                new MaidAction(MaidAction.Kind.GO_TO, MaidAction.Target.PLAYER));
        if (!accepted(frame)) {
            source.sendFailure(Component.literal("她没能开始走过来：" + rejectReason(frame)));
            return 0;
        }
        TestFeedback.expectAction(player, maid);
        source.sendSuccess(() -> Component.literal(
                "开始走过来；结束原因会发到聊天栏"), false);
        return 1;
    }

    /**
     * 直接用固定图纸开一次工：小木屋或垫脚测试柱，位置按执行者的准星定。
     *
     * <p>图纸已经盖好、或者一块都放不了时施工器会当场收工，那种情况直接报结论。
     */
    private static int testBuild(CommandSourceStack source, boolean house) {
        ServerPlayer player = source.getPlayer();
        if (player == null) {
            source.sendFailure(Component.literal("这条要玩家自己执行：图纸按准星定位置"));
            return 0;
        }
        EntityMaid maid = nearest(source);
        if (maid == null) {
            source.sendFailure(Component.literal("她不在附近（要 64 格内，且区块已加载）"));
            return 0;
        }
        if (player.level() != maid.level()) {
            source.sendFailure(Component.literal("你和她在不同的世界，图纸没法落在你眼前"));
            return 0;
        }
        BlockHitResult gaze = MaidQueries.gazeHit(player, MineRules.ANCHOR_RAY);
        if (gaze == null) {
            source.sendFailure(Component.literal("先看向要盖的那片地面，再执行"));
            return 0;
        }
        ServerLevel level = (ServerLevel) maid.level();
        BlockPos anchor = gaze.getBlockPos();
        if (!level.isLoaded(anchor)) {
            source.sendFailure(Component.literal("准星那一格还没加载出来"));
            return 0;
        }
        // 先按 y=0 排一份算地面，再按算出来的起点正式生成图纸
        List<MaidBuildController.Target> draft = house
                ? TestBlueprints.house(anchor, 0)
                : TestBlueprints.climb(anchor, 0);
        int baseY = TestBlueprints.baseY(level, anchor, draft);
        List<MaidBuildController.Target> targets = house
                ? TestBlueprints.house(anchor, baseY)
                : TestBlueprints.climb(anchor, baseY);
        for (MaidBuildController.Target target : targets) {
            if (!level.isLoaded(target.pos())) {
                source.sendFailure(Component.literal("图纸有一格还没加载出来"));
                return 0;
            }
            if (maid.hasRestriction() && !maid.isWithinRestriction(target.pos())) {
                source.sendFailure(Component.literal("图纸超出了她的活动范围"));
                return 0;
            }
        }
        ServerPlayer owner = MaidQueries.owner(maid);
        boolean consume = owner == null || !owner.isCreative();
        String name = house ? "小木屋" : "垫脚测试柱";
        PathDebug.show(maid, EnumSet.allOf(PathDebug.Layer.class));
        MaidBuildController.start(source.getServer(), maid, name,
                "tlml-test-" + (house ? "house" : "climb"), anchor, baseY, targets, consume);
        if (MaidBuildController.isRunning(maid)) {
            TestFeedback.expectBuild(player, maid, name);
            source.sendSuccess(() -> Component.literal(
                    "开始盖" + name + "（" + targets.size() + " 块）；结束原因会发到聊天栏"), false);
        } else {
            TestFeedback.buildNow(player, MaidBuildController.status());
        }
        return 1;
    }

    /** 动作受理回执里 accepted 那一项；理论上总是在 data 里，缺了按没受理算。 */
    private static boolean accepted(JsonObject frame) {
        JsonObject data = Envelope.data(frame);
        return data.has("accepted") && data.get("accepted").getAsBoolean();
    }

    private static String rejectReason(JsonObject frame) {
        String reason = Envelope.string(Envelope.data(frame), "reason");
        return reason.isEmpty() ? "她没能开始这次动作" : reason;
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
