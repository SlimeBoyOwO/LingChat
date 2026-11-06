package com.lingchat.maidbridge.action.mine;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.action.core.MaidActionController;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.action.attack.MaidAttackController;
import com.lingchat.maidbridge.action.place.MaidPlaceController;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.bridge.Protocol;
import com.lingchat.maidbridge.maid.MaidCondition;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.nearby.Surroundings;
import com.lingchat.maidbridge.perception.query.BlockProjector;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.state.BlockState;

/**
 * {@code maid_mine}：让她去挖方块。
 *
 * <p>目标有三种说法：{@code looked}（玩家准星指着的方向，玩家说「把这块挖了」时用）、
 * 一个方块注册表 id（「挖十块石头」），或地址 {@code at}（相对她脚下那一格的偏移，
 * 「看一眼周围」的关系句给出）。插件把中文名翻成 id。
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
        if (MaidCondition.isUnconscious(maid)) {
            return rejected("maid_unconscious", "她昏迷不醒，叫不动", "", 0);
        }
        if (MaidQueries.carriedByPlayer(maid)) {
            return rejected("maid_carried", "玩家正抱着她，先放下来再让她挖", "", 0);
        }
        JsonObject data = Envelope.data(request);
        return mine(
                server, maid, Envelope.string(data, "target"), parseCount(data),
                Envelope.string(data, "at"), Envelope.string(data, "area"),
                parseFlag(data, "whole"));
    }

    /**
     * 三条入口：一片区域（两个角）、整团同种方块（{@code whole}），或原来那几种单块/若干块。
     *
     * @param rawArea 长方体的两个相对角，非空时优先于其余写法
     * @param whole   以目标那一格为种子，把相连的一团同种方块整团拆掉
     */
    private static JsonObject mine(
            MinecraftServer server, EntityMaid maid, String rawTarget, int count, String rawAt,
            String rawArea, boolean whole) {
        if (rawArea != null && !rawArea.trim().isEmpty()) {
            return mineArea(server, maid, rawArea.trim(), count);
        }
        if (!rawAt.isEmpty()) {
            return mineAddressed(server, maid, rawTarget, count, rawAt, whole);
        }
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
            blockId = blockKind(token);
            if (blockId == null) {
                return rejected("bad_target", "没听懂要挖哪种方块", token, count);
            }
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
            return rejected(
                    "not_mineable", "那不是能挖的方块（水、岩浆这类不算）",
                    blockId, count, scene(maid, target));
        }
        if (!MineRules.mineable(state.getDestroySpeed(maid.level(), target))) {
            return rejected(
                    "not_mineable", "这个方块挖不动", blockId, count, scene(maid, target));
        }
        if (!maid.canDestroyBlock(target)) {
            return rejected(
                    "protected", "这个方块不让她挖（受保护）", blockId, count, scene(maid, target));
        }
        if (!MineTargets.hasCorrectTool(maid, state)) {
            return rejected(
                    "wrong_tool", MineTargets.missingToolReason(maid, state),
                    blockId, count, scene(maid, target));
        }
        // 没有落脚点也受理：埋在墙里的矿、树干高处的木头，她会一路把挡路的方块挖穿过去。
        // 真正走不通时由控制器在挖的过程中收尾，理由比「走不到跟前」具体。
        if (whole) {
            return startWhole(server, maid, target, state, blockId, null, looked);
        }
        String replaced = stopOtherActions(server, maid);
        MaidMineController.start(maid, target, blockId, count, anchor);

        return accepted(maid, state, blockId, count, looked, replaced);
    }

    /**
     * 按相对地址挖：{@code at} 相对她脚下那一格，由「看一眼周围」的关系句给出。
     *
     * <p>三处核对：那一格是不是要挖的方块、她知不知道那一格、够不够得着能挖。对不上就带着
     * 现场与最近的同类方块回去，让模型改个地址或改个说法，而不是照原样重试。
     */
    private static JsonObject mineAddressed(
            MinecraftServer server, EntityMaid maid, String rawTarget, int count, String rawAt,
            boolean whole) {
        MineRules.Offset offset = MineRules.parseOffset(rawAt);
        if (offset == null) {
            return rejected(
                    "bad_address",
                    "没看懂这个地址：" + rawAt + "。它应该像 @4,-2,-3，是相对她脚下那一格的偏移",
                    rawTarget, count);
        }
        Level level = maid.level();
        BlockPos target = maid.blockPosition().offset(offset.dx(), offset.dy(), offset.dz());
        if (!level.isLoaded(target)) {
            return atRejected(
                    "not_loaded", "那一格还没加载出来", "", count,
                    scene(maid, target), null, offset);
        }
        String token = rawTarget == null ? "" : rawTarget.trim();
        // 地址已经指到某一格时，target 只用来核对种类；没给就是「那一格原本是什么就挖什么」
        boolean unkind = token.isEmpty() || "looked".equalsIgnoreCase(token);
        BlockState state = level.getBlockState(target);
        boolean voidCell = state.isAir() || state.getBlock() instanceof LiquidBlock;
        String blockId;
        if (unkind) {
            blockId = voidCell ? "" : MineTargets.blockId(state);
        } else {
            blockId = blockKind(token);
            if (blockId == null) {
                return rejected("bad_target", "没听懂要挖哪种方块", token, count);
            }
        }
        if (voidCell) {
            return atRejected(
                    "not_that_block", "那一格现在是空的（水、岩浆也算空）",
                    blockId, count, scene(maid, target), nearestOf(maid, blockId), offset);
        }
        if (!unkind && !MineTargets.targets(blockId).contains(state.getBlock())) {
            return atRejected(
                    "not_that_block", "那一格不是要挖的那种方块",
                    blockId, count, scene(maid, target), nearestOf(maid, blockId), offset);
        }
        if (!MineTargets.sheKnows(maid, target, state)) {
            return atRejected(
                    "not_seen", "她没看见过那一格",
                    blockId, count, scene(maid, target), null, offset);
        }
        if (!MineRules.mineable(state.getDestroySpeed(level, target))) {
            return atRejected(
                    "not_mineable", "这个方块挖不动",
                    blockId, count, scene(maid, target), null, offset);
        }
        if (!maid.canDestroyBlock(target)) {
            return atRejected(
                    "protected", "这个方块不让她挖（受保护）",
                    blockId, count, scene(maid, target), null, offset);
        }
        if (!MineTargets.hasCorrectTool(maid, state)) {
            return atRejected(
                    "wrong_tool", MineTargets.missingToolReason(maid, state),
                    blockId, count, scene(maid, target), null, offset);
        }

        if (whole) {
            return startWhole(server, maid, target, state, blockId, offset, false);
        }
        String replaced = stopOtherActions(server, maid);
        // 锚点就是她指的那一格：要挖多块时以它为中心接着找同类方块
        MaidMineController.start(maid, target, blockId, count, target);

        JsonObject frame = accepted(maid, state, blockId, count, false, replaced);
        JsonObject data = Envelope.data(frame);
        data.addProperty("addressed", true);
        data.addProperty("at", MineRules.formatOffset(offset.dx(), offset.dy(), offset.dz()));
        data.add("scene", scene(maid, target));
        return frame;
    }

    /**
     * 整团拆：以 seed 为起点按六面相连收同种方块，再逐块交给控制器。
     *
     * <p>区域拆除与整团拆都走「格子清单」那条路；这里只负责把这一团挑出来，并把拆不动的
     * 方块（受保护、缺工具）先剔掉，免得拆到一半才停下。
     */
    private static JsonObject startWhole(
            MinecraftServer server, EntityMaid maid, BlockPos seed, BlockState seedState,
            String blockId, MineRules.Offset offset, boolean looked) {
        Level level = maid.level();
        MineTargets.Cells group = MineTargets.cluster(maid, seed, blockId, MineRules.MAX_AREA_CELLS);
        List<BlockPos> cells = new ArrayList<>();
        for (BlockPos pos : group.cells()) {
            if (!maid.canDestroyBlock(pos)) {
                continue;
            }
            BlockState one = level.getBlockState(pos);
            if (one.requiresCorrectToolForDrops() && !MineTargets.hasCorrectTool(maid, one)) {
                continue;
            }
            cells.add(pos);
        }
        if (cells.isEmpty()) {
            String reason = "这一团方块她都拆不了（受保护或者缺工具）";
            if (offset == null) {
                return rejected("not_mineable", reason, blockId, 0);
            }
            return atRejected(
                    "not_mineable", reason, blockId, 0, scene(maid, seed), null, offset);
        }
        String replaced = stopOtherActions(server, maid);
        MaidMineController.startCells(maid, cells, group.truncated(), blockId);
        JsonObject frame = acceptedCells(
                maid, blockId, seedState.getBlock().getName().getString(),
                cells.size(), group.truncated(), looked, replaced);
        if (offset != null) {
            JsonObject data = Envelope.data(frame);
            data.addProperty("addressed", true);
            data.addProperty("at", MineRules.formatOffset(offset.dx(), offset.dy(), offset.dz()));
            data.add("scene", scene(maid, seed));
        }
        return frame;
    }

    /**
     * 拆一片长方体：两个相对角由模型给，偏移都相对她脚下那一格。
     *
     * <p>只收能拆的方块：空气、液体、挖不动的、受保护、缺工具的都不进清单；清单按离她的
     * 远近取前 {@link MineRules#MAX_AREA_CELLS} 块，再多就只拆最近的这些，回执里说一声。
     */
    private static JsonObject mineArea(
            MinecraftServer server, EntityMaid maid, String rawArea, int count) {
        MineRules.Area area = MineRules.parseArea(rawArea);
        if (area == null) {
            return rejected(
                    "bad_address",
                    "没看懂这个区域：" + rawArea + "。它应该像 @-3,0,2 @3,3,4，是两个相对她脚下那一格的角",
                    rawArea, count);
        }
        Level level = maid.level();
        BlockPos feet = maid.blockPosition();
        int minX = Math.min(area.from().dx(), area.to().dx());
        int minY = Math.min(area.from().dy(), area.to().dy());
        int minZ = Math.min(area.from().dz(), area.to().dz());
        int maxX = Math.max(area.from().dx(), area.to().dx());
        int maxY = Math.max(area.from().dy(), area.to().dy());
        int maxZ = Math.max(area.from().dz(), area.to().dz());
        List<BlockPos> found = new ArrayList<>();
        for (int x = minX; x <= maxX; x++) {
            for (int y = minY; y <= maxY; y++) {
                for (int z = minZ; z <= maxZ; z++) {
                    BlockPos pos = feet.offset(x, y, z);
                    if (!level.isLoaded(pos)) {
                        continue;
                    }
                    BlockState state = level.getBlockState(pos);
                    if (state.isAir() || state.getBlock() instanceof LiquidBlock) {
                        continue;
                    }
                    if (!MineRules.mineable(state.getDestroySpeed(level, pos))) {
                        continue;
                    }
                    if (maid.hasRestriction() && !maid.isWithinRestriction(pos)) {
                        continue;
                    }
                    found.add(pos);
                }
            }
        }
        if (found.isEmpty()) {
            return rejected("not_mineable", "这一片里没有能拆的方块", rawArea, count);
        }
        found.sort(Comparator.comparingDouble(feet::distSqr));
        List<BlockPos> cells = new ArrayList<>();
        for (BlockPos pos : found) {
            if (cells.size() >= MineRules.MAX_AREA_CELLS) {
                break;
            }
            if (!maid.canDestroyBlock(pos)) {
                continue;
            }
            BlockState state = level.getBlockState(pos);
            if (state.requiresCorrectToolForDrops() && !MineTargets.hasCorrectTool(maid, state)) {
                continue;
            }
            cells.add(pos);
        }
        if (cells.isEmpty()) {
            return rejected("not_mineable", "这一片都拆不了（受保护或者缺工具）", rawArea, count);
        }
        boolean truncated = cells.size() >= MineRules.MAX_AREA_CELLS && found.size() > cells.size();
        String common = commonBlockId(level, cells);
        String replaced = stopOtherActions(server, maid);
        MaidMineController.startCells(maid, cells, truncated, common);
        return acceptedCells(
                maid, common, blockName(common), cells.size(), truncated, false, replaced);
    }

    /** 区域拆除的受理回执：与 accepted 同样的外形，另带 area / cells / truncated。 */
    private static JsonObject acceptedCells(
            EntityMaid maid, String blockId, String name, int cells, boolean truncated,
            boolean looked, String replaced) {
        JsonObject data = base();
        data.addProperty("accepted", true);
        data.addProperty("async", true);
        data.addProperty("looked", looked);
        data.addProperty("addressed", false);
        data.addProperty("at", "");
        data.addProperty("count", cells);
        data.addProperty("block", blockId);
        data.addProperty("name", name);
        data.addProperty("requires_tool", false);
        data.addProperty("has_tool", true);
        data.addProperty("tool", handItemId(maid));
        data.addProperty("replaced_action", replaced);
        data.addProperty("area", true);
        data.addProperty("cells", cells);
        data.addProperty("truncated", truncated);
        return Envelope.response(Protocol.MAID_MINE_RESULT, data);
    }

    /** 开新的挖掘之前把别的会写寻路的都收掉：动作、攻击与上一轮挖掘。 */
    private static String stopOtherActions(MinecraftServer server, EntityMaid maid) {
        MaidAttackController.cancel();
        String replaced = MaidActionController.cancelCurrent(server);
        // 上一轮挖掘留下的裂纹与寻路也一并清掉，再开始新的
        MaidMineController.cancel(server);
        MaidPlaceController.cancel(server);
        MaidActuator.standUp(maid);
        return replaced;
    }

    /** 清单里只有一种方块时给它的 id，混着几种给空串。 */
    private static String commonBlockId(Level level, List<BlockPos> cells) {
        String common = "";
        for (BlockPos pos : cells) {
            String id = MineTargets.blockId(level.getBlockState(pos));
            if (common.isEmpty()) {
                common = id;
            } else if (!common.equals(id)) {
                return "";
            }
        }
        return common;
    }

    private static String blockName(String blockId) {
        if (blockId.isEmpty()) {
            return "";
        }
        ResourceLocation id = ResourceLocation.tryParse(blockId);
        Block block = id == null ? null : BuiltInRegistries.BLOCK.get(id);
        return block == null ? "" : block.getName().getString();
    }

    private static boolean parseFlag(JsonObject data, String key) {
        try {
            return data.get(key).getAsBoolean();
        } catch (RuntimeException missingOrNotABoolean) {
            return false;
        }
    }

    /** 目标写法 → 规范写法：标签保留 {@code #}，裸名补 minecraft:；认不出给 null。 */
    private static String blockKind(String token) {
        boolean tagged = token.startsWith("#");
        String bare = tagged ? token.substring(1) : token;
        ResourceLocation id =
                ResourceLocation.tryParse(bare.contains(":") ? bare : "minecraft:" + bare);
        if (id == null) {
            return null;
        }
        return tagged ? "#" + id : id.toString();
    }

    /** 受理成功：两条路径的字段一样，地址那条再多出 addressed、at 与 scene。 */
    private static JsonObject accepted(
            EntityMaid maid, BlockState state, String blockId, int count, boolean looked,
            String replaced) {
        JsonObject data = base();
        data.addProperty("accepted", true);
        data.addProperty("async", true);
        data.addProperty("looked", looked);
        data.addProperty("addressed", false);
        data.addProperty("at", "");
        data.addProperty("count", count);
        data.addProperty("block", blockId);
        data.addProperty("name", state.getBlock().getName().getString());
        data.addProperty("requires_tool", state.requiresCorrectToolForDrops());
        data.addProperty("has_tool", MineTargets.hasCorrectTool(maid, state));
        data.addProperty("tool", handItemId(maid));
        data.addProperty("replaced_action", replaced);
        return Envelope.response(Protocol.MAID_MINE_RESULT, data);
    }

    /** 目标相对她的位置，以及周围站不站得下、上方压没压着东西、视线隔了几块。 */
    private static JsonObject scene(EntityMaid maid, BlockPos target) {
        JsonObject scene = new JsonObject();
        Level level = maid.level();
        BlockState state = level.getBlockState(target);
        boolean empty = state.isAir();
        scene.addProperty("id", empty ? "" : MineTargets.blockId(state));
        scene.addProperty("name", empty ? "" : state.getBlock().getName().getString());
        BlockPos feet = maid.blockPosition();
        int dx = target.getX() - feet.getX();
        int dy = target.getY() - feet.getY();
        int dz = target.getZ() - feet.getZ();
        scene.addProperty("dx", dx);
        scene.addProperty("dy", dy);
        scene.addProperty("dz", dz);
        scene.addProperty("at", MineRules.formatOffset(dx, dy, dz));
        scene.addProperty(
                "facing",
                Surroundings.direction(
                        maid.getYRot(), target.getX() - maid.getX(), target.getZ() - maid.getZ()));
        scene.addProperty(
                "blockers", BlockProjector.blockerCount(level, maid.getEyePosition(), target));
        scene.addProperty(
                "blocker", BlockProjector.blockerId(level, maid.getEyePosition(), target));
        BlockPos above = target.above();
        BlockState aboveState = level.getBlockState(above);
        scene.addProperty(
                "blocked_above",
                !aboveState.isAir() && !aboveState.canBeReplaced()
                        && !aboveState.getCollisionShape(level, above).isEmpty());
        scene.addProperty("stand_room", MineTargets.standRoom(maid, target));
        return scene;
    }

    /**
     * 指错时最近的同类方块：只挑她看见过的，带上它自己的地址，下次照着指就能挖到。
     *
     * <p>全都看不见时给 null——她本来就不知道哪有，不能拿全知扫描替她编一个位置。
     */
    private static JsonObject nearestOf(EntityMaid maid, String blockId) {
        if (blockId == null || blockId.isEmpty()) {
            return null;
        }
        List<BlockPos> candidates = MineTargets.nearest(
                maid, maid.blockPosition(), blockId, MineRules.FIND_RADIUS, MineRules.FIND_LIMIT);
        BlockPos fallback = null;
        for (BlockPos candidate : candidates) {
            if (!maid.canDestroyBlock(candidate)) {
                continue;
            }
            BlockState state = maid.level().getBlockState(candidate);
            if (!MineTargets.sheKnows(maid, candidate, state)) {
                continue;
            }
            if (MineTargets.standFor(maid, candidate) != null) {
                return scene(maid, candidate);
            }
            if (fallback == null) {
                fallback = candidate;
            }
        }
        return fallback == null ? null : scene(maid, fallback);
    }

    /**
     * 以 center 为中心找一块这个 id 的方块。
     *
     * <p>先挑有落脚点的；全都被埋着（矿脉、树干心）就退回最近的那一块——挖掘控制器会先
     * 把挡路的方块挖穿再动手。原来只认有落脚点的，于是「看一眼周围」明明列着深层铁矿石，
     * 挖的时候却回「没有能挖到的这种方块」，模型只能一遍遍重试。
     */
    private static BlockPos findTargetNear(EntityMaid maid, String blockId, BlockPos center, int radius) {
        List<BlockPos> candidates =
                MineTargets.nearest(maid, center, blockId, radius, MineRules.FIND_LIMIT);
        BlockPos nearest = null;
        for (BlockPos candidate : candidates) {
            if (!maid.canDestroyBlock(candidate)) {
                continue;
            }
            if (MineTargets.standFor(maid, candidate) != null) {
                return candidate;
            }
            if (nearest == null) {
                nearest = candidate;
            }
        }
        return nearest;
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
        return rejected(reasonCode, reason, target, count, null, null);
    }

    private static JsonObject rejected(
            String reasonCode, String reason, String target, int count, JsonObject scene) {
        return rejected(reasonCode, reason, target, count, scene, null);
    }

    private static JsonObject rejected(
            String reasonCode, String reason, String target, int count, JsonObject scene,
            JsonObject nearest) {
        JsonObject data = base();
        data.addProperty("accepted", false);
        data.addProperty("async", false);
        data.addProperty("looked", "looked".equalsIgnoreCase(target) || target.isEmpty());
        data.addProperty("addressed", false);
        data.addProperty("at", "");
        data.addProperty("count", count);
        data.addProperty("block", target == null ? "" : target);
        data.addProperty("name", "");
        data.addProperty("requires_tool", false);
        data.addProperty("has_tool", false);
        data.addProperty("tool", "");
        data.addProperty("replaced_action", "");
        data.addProperty("reason_code", reasonCode);
        data.addProperty("reason", reason);
        if (scene != null) {
            data.add("scene", scene);
        }
        if (nearest != null) {
            data.add("nearest", nearest);
        }
        return Envelope.response(Protocol.MAID_MINE_RESULT, data);
    }

    /** 地址那条路上的拒绝：回执也带上地址，插件据此知道模型指的是哪一块。 */
    private static JsonObject atRejected(
            String reasonCode, String reason, String target, int count, JsonObject scene,
            JsonObject nearest, MineRules.Offset offset) {
        JsonObject frame = rejected(reasonCode, reason, target, count, scene, nearest);
        JsonObject data = Envelope.data(frame);
        data.addProperty("addressed", true);
        data.addProperty("at",
                MineRules.formatOffset(offset.dx(), offset.dy(), offset.dz()));
        return frame;
    }
}
