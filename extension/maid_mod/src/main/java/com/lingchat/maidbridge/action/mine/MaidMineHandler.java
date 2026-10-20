package com.lingchat.maidbridge.action.mine;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.action.core.MaidActionController;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.action.attack.MaidAttackController;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.bridge.Protocol;
import com.lingchat.maidbridge.maid.MaidQueries;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.state.BlockState;

/**
 * {@code maid_mine}：让她去挖方块。
 *
 * <p>目标只有两种说法：{@code looked}（玩家准星指着的方向，玩家说「把这块挖了」时用）或
 * 一个方块注册表 id（「挖十块石头」）。插件把中文名翻成 id。
 *
 * <p>玩家准星指着哪个方向，就先在落点附近那一片找同种方块；准星没指着、或那一片里没有，
 * 才退回以她为中心找。{@code looked} 则直接挖落点那一块，并接着在那一片里补够数量——
 * 玩家指一片林子，她就该在那片林子里砍，而不是按「离她最近」跑到别处去。
 *
 * <p>落脚点与寻路交给 {@link MaidMineController} 每 tick 推进。
 *
 * <p>受理时就能判定的失败直接回给插件：方块不存在、挖不动、受保护、她走不到、没有合适的
 * 工具。受理之后是异步的——挖穿一块、收尾、走不到都会推 {@code maid_mine_done}。
 */
public class MaidMineHandler implements MessageHandler {
    private final MinecraftServer server;

    public MaidMineHandler(MinecraftServer server) {
        this.server = server;
    }

    @Override
    public JsonObject handle(JsonObject request) {
        if (server == null) {
            return Envelope.error(MaidQueries.NOT_READY);
        }
        EntityMaid maid = MaidQueries.findMaidById(server, Envelope.string(Envelope.data(request), "maid_id"));
        if (maid == null) {
            return rejected("maid_not_found", MaidQueries.NOT_FOUND, "", 0);
        }
        if (MaidQueries.carriedByPlayer(maid)) {
            return rejected("maid_carried", "玩家正抱着她，先放下来再让她挖", "", 0);
        }
        JsonObject data = Envelope.data(request);
        return mine(server, maid, Envelope.string(data, "target"), parseCount(data));
    }

    private static JsonObject mine(MinecraftServer server, EntityMaid maid, String rawTarget, int count) {
        String token = rawTarget == null ? "" : rawTarget.trim();
        boolean looked = token.isEmpty() || "looked".equalsIgnoreCase(token);
        ServerPlayer owner = MaidQueries.owner(maid);
        boolean ownerHere = owner != null && owner.level() == maid.level();
        // 玩家准星落在哪：不要求凑到交互距离内，指着远处的林子或矿壁也算
        BlockPos gaze = ownerHere ? MaidQueries.gazeAt(owner, MineRules.ANCHOR_RAY) : null;
        BlockPos target = null;
        BlockPos anchor = null;
        String blockId;
        if (looked) {
            if (!ownerHere) {
                return rejected("player_away", "玩家不在这里，没法知道要挖哪一块", "", count);
            }
            if (gaze == null) {
                return rejected("no_target", "玩家看着的方向上没有方块", "", count);
            }
            blockId = MineTargets.blockId(maid.level().getBlockState(gaze));
            // 落点那一块先挖，剩下的在这片里接着找
            target = gaze;
            anchor = gaze;
        } else {
            // 「#minecraft:logs」是方块标签（泛称），方括号里的 id 仍按普通方块解析
            boolean tagged = token.startsWith("#");
            String bare = tagged ? token.substring(1) : token;
            ResourceLocation id = ResourceLocation.tryParse(bare.contains(":") ? bare : "minecraft:" + bare);
            if (id == null) {
                return rejected("bad_target", "没听懂要挖哪种方块", token, count);
            }
            blockId = tagged ? "#" + id : id.toString();
            // 玩家指着的那片有这种方块就去那片；没指着或那片没有才退回她身边
            if (gaze != null) {
                target = findTargetNear(maid, blockId, gaze, MineRules.ANCHOR_RANGE);
                if (target != null) {
                    anchor = gaze;
                }
            }
            if (target == null) {
                target = findTargetNear(maid, blockId, maid.blockPosition(), MineRules.FIND_RADIUS);
            }
            if (target == null) {
                return rejected(
                        "no_target",
                        gaze != null ? "你指的那片和她附近都没有能挖到的这种方块" : "她附近没有能挖到的这种方块",
                        token,
                        count);
            }
        }

        BlockState state = maid.level().getBlockState(target);
        if (state.isAir() || state.getBlock() instanceof LiquidBlock) {
            return rejected("not_mineable", "那不是能挖的方块（水、岩浆这类不算）", blockId, count);
        }
        if (!MineRules.mineable(state.getDestroySpeed(maid.level(), target))) {
            return rejected("not_mineable", "这个方块挖不动", blockId, count);
        }
        if (!maid.canDestroyBlock(target)) {
            return rejected("protected", "这个方块不让她挖（受保护）", blockId, count);
        }
        if (!MineTargets.hasCorrectTool(maid, state)) {
            return rejected("wrong_tool", "她手上和背包里都没有能挖它的工具，得先给她一把", blockId, count);
        }
        if (MineTargets.standFor(maid, target) == null) {
            return rejected("unreachable", "那块方块她走不到跟前，先走近一点", blockId, count);
        }

        // 插件发起的动作与攻击每 tick 都在写寻路，会跟挖掘互相顶
        MaidAttackController.cancel();
        String replaced = MaidActionController.cancelCurrent(server);
        // 上一轮挖掘留下的裂纹与寻路也一并清掉，再开始新的
        MaidMineController.cancel(server);
        MaidActuator.standUp(maid);
        MaidMineController.start(maid, target, blockId, count, anchor);

        JsonObject data = base();
        data.addProperty("accepted", true);
        data.addProperty("async", true);
        data.addProperty("looked", looked);
        data.addProperty("count", count);
        data.addProperty("block", blockId);
        data.addProperty("name", state.getBlock().getName().getString());
        data.addProperty("requires_tool", state.requiresCorrectToolForDrops());
        data.addProperty("has_tool", MineTargets.hasCorrectTool(maid, state));
        data.addProperty("tool", handItemId(maid));
        data.addProperty("replaced_action", replaced);
        return Envelope.response(Protocol.MAID_MINE_RESULT, data);
    }

    /** 以 center 为中心找一块这个 id 的、走得到跟前的方块。 */
    private static BlockPos findTargetNear(EntityMaid maid, String blockId, BlockPos center, int radius) {
        List<BlockPos> candidates =
                MineTargets.nearest(maid, center, blockId, radius, MineRules.FIND_LIMIT);
        for (BlockPos candidate : candidates) {
            if (!maid.canDestroyBlock(candidate)) {
                continue;
            }
            if (MineTargets.standFor(maid, candidate) != null) {
                return candidate;
            }
        }
        return null;
    }

    private static int parseCount(JsonObject data) {
        try {
            return MineRules.clampCount(data.get("count").getAsInt());
        } catch (RuntimeException missingOrNotANumber) {
            return 1;
        }
    }

    private static String handItemId(EntityMaid maid) {
        return maid.getMainHandItem().isEmpty()
                ? ""
                : BuiltInRegistries.ITEM.getKey(maid.getMainHandItem().getItem()).toString();
    }

    private static JsonObject base() {
        JsonObject data = new JsonObject();
        data.addProperty("reason_code", "");
        data.addProperty("reason", "");
        return data;
    }

    private static JsonObject rejected(String reasonCode, String reason, String target, int count) {
        JsonObject data = base();
        data.addProperty("accepted", false);
        data.addProperty("async", false);
        data.addProperty("looked", "looked".equalsIgnoreCase(target) || target.isEmpty());
        data.addProperty("count", count);
        data.addProperty("block", target == null ? "" : target);
        data.addProperty("name", "");
        data.addProperty("requires_tool", false);
        data.addProperty("has_tool", false);
        data.addProperty("tool", "");
        data.addProperty("replaced_action", "");
        data.addProperty("reason_code", reasonCode);
        data.addProperty("reason", reason);
        return Envelope.response(Protocol.MAID_MINE_RESULT, data);
    }
}
