package com.lingchat.maidbridge.action.place;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.action.attack.MaidAttackController;
import com.lingchat.maidbridge.action.core.MaidActionController;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.action.mine.MaidMineController;
import com.lingchat.maidbridge.action.mine.MineRules;
import com.lingchat.maidbridge.action.mine.MineTargets;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.bridge.Protocol;
import com.lingchat.maidbridge.maid.MaidCondition;
import com.lingchat.maidbridge.maid.MaidInventory;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.nearby.Surroundings;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.BlockHitResult;

/**
 * {@code maid_place}：让她把一块方块放到玩家指着的那一面，或一个相对地址声明的空格。
 *
 * <p>两条入口：没有 {@code at} 时按主人准星看到的方块面放（与玩家自己右键一致，主人说
 * 「在这放个方块」时用它）；给了 {@code at} 就把那一格当目标，模型可以从「看一眼周围」
 * 的地址往旁边推一格。受理前核：那一格能不能替换、有没有能贴着的面、有没有活物占着、
 * 她身上有没有这种方块；放置本身交给 {@link MaidPlaceController} 走过去按原版规则完成。
 */
public class MaidPlaceHandler implements MessageHandler {
    private final MinecraftServer server;

    public MaidPlaceHandler(MinecraftServer server) {
        this.server = server;
    }

    @Override
    public JsonObject handle(JsonObject request) {
        if (server == null) {
            return Envelope.error(MaidQueries.NOT_READY);
        }
        JsonObject data = Envelope.data(request);
        EntityMaid maid = MaidQueries.findMaidById(server, Envelope.string(data, "maid_id"));
        if (maid == null) {
            return rejected("maid_not_found", MaidQueries.NOT_FOUND, "", "", "", null);
        }
        if (MaidCondition.isUnconscious(maid)) {
            return rejected("maid_unconscious", "她昏迷不醒，叫不动", "", "", "", null);
        }
        if (MaidQueries.carriedByPlayer(maid)) {
            return rejected("maid_carried", "玩家正抱着她，先放下来再让她放", "", "", "", null);
        }
        return place(server, maid, data);
    }

    private static JsonObject place(
            MinecraftServer server, EntityMaid maid, JsonObject data) {
        String rawItem = Envelope.string(data, "item");
        String rawAt = Envelope.string(data, "at");
        String rawFacing = Envelope.string(data, "facing");

        String blockId = blockKind(rawItem);
        if (blockId == null) {
            return rejected(
                    "bad_target", "没听懂要放哪种方块（要写具体方块，不能用 # 标签）",
                    rawItem, rawAt, rawFacing, null);
        }
        ResourceLocation id = ResourceLocation.tryParse(blockId);
        Block block = id == null ? Blocks.AIR : BuiltInRegistries.BLOCK.get(id);
        if (block == Blocks.AIR) {
            return rejected("not_placeable", "没有这种方块", rawItem, rawAt, rawFacing, null);
        }
        Item item = block.asItem();
        if (item == net.minecraft.world.item.Items.AIR || !(item instanceof BlockItem)) {
            return rejected(
                    "not_placeable", "这个方块没有能放下的物品",
                    rawItem, rawAt, rawFacing, null);
        }

        Level level = maid.level();
        BlockPos target;
        BlockPos preferredSupport = null;
        Direction preferredFace = null;
        boolean looked = rawAt.isEmpty();
        if (!looked) {
            MineRules.Offset offset = MineRules.parseOffset(rawAt);
            if (offset == null) {
                return rejected(
                        "bad_address",
                        "没看懂这个地址：" + rawAt + "。它应该像 @3,1,1，是相对她脚下那一格的偏移",
                        rawItem, rawAt, rawFacing, null);
            }
            target = maid.blockPosition().offset(offset.dx(), offset.dy(), offset.dz());
        } else {
            ServerPlayer owner = MaidQueries.owner(maid);
            boolean ownerHere = owner != null && owner.level() == maid.level();
            if (!ownerHere) {
                return rejected(
                        "player_away", "玩家不在这里，不知道要放在哪", rawItem, rawAt, rawFacing, null);
            }
            BlockHitResult gaze = MaidQueries.gazeHit(owner, MineRules.ANCHOR_RAY);
            if (gaze == null) {
                return rejected(
                        "no_target", "玩家看着的方向上没有能放的地方",
                        rawItem, rawAt, rawFacing, null);
            }
            preferredSupport = gaze.getBlockPos();
            preferredFace = gaze.getDirection();
            BlockState clicked = level.getBlockState(preferredSupport);
            // 与玩家右键一致：点着草、雪这类可替换的方块，就是把它换成手里的方块
            target = clicked.canBeReplaced()
                    ? preferredSupport
                    : preferredSupport.relative(preferredFace);
        }

        if (!level.isLoaded(target)) {
            return rejected(
                    "not_loaded", "那一格还没加载出来", rawItem, rawAt, rawFacing, scene(maid, target));
        }
        if (maid.hasRestriction() && !maid.isWithinRestriction(target)) {
            return rejected(
                    "out_of_restriction", "那一格不在她的活动范围里",
                    rawItem, rawAt, rawFacing, scene(maid, target));
        }
        BlockState targetState = level.getBlockState(target);
        Float yaw = PlaceRules.yawOf(rawFacing);
        BlockPlaceContext context =
                Placement.context(maid, new ItemStack(item), target, preferredSupport, preferredFace, yaw);
        if (context == null) {
            boolean replaceable = targetState.isAir() || targetState.canBeReplaced();
            return rejected(
                    replaceable ? "no_support" : "not_replaceable",
                    replaceable ? "那一格旁边没有能贴着放的面" : "那一格已经被别的东西占了",
                    rawItem, rawAt, rawFacing, scene(maid, target));
        }
        if (occupiedByOther(level, target, maid)) {
            return rejected(
                    "occupied", "那一格里站着别的人或动物",
                    rawItem, rawAt, rawFacing, scene(maid, target));
        }
        if (!MaidInventory.has(maid, item)) {
            return rejected(
                    "no_item", "她身上没有这种方块",
                    rawItem, rawAt, rawFacing, scene(maid, target));
        }

        String replaced = stopOtherActions(server, maid);
        MaidPlaceController.start(
                maid, target, blockId, itemId(item), block, preferredSupport, preferredFace, yaw);
        return accepted(
                maid, target, blockId, itemId(item), block.getName().getString(),
                looked, yaw, replaced);
    }

    /** 开新的放置之前把别的会写寻路的都收掉：动作、攻击、挖掘与上一轮放置。 */
    private static String stopOtherActions(MinecraftServer server, EntityMaid maid) {
        MaidAttackController.cancel();
        String replaced = MaidActionController.cancelCurrent(server);
        MaidMineController.cancel(server);
        MaidPlaceController.cancel(server);
        MaidActuator.standUp(maid);
        return replaced;
    }

    /** 目标写法 → 规范写法：裸名补 minecraft:；认不出或标签给 null。 */
    private static String blockKind(String token) {
        String bare = token == null ? "" : token.trim();
        if (bare.isEmpty() || bare.startsWith("#")) {
            return null;
        }
        ResourceLocation id =
                ResourceLocation.tryParse(bare.contains(":") ? bare : "minecraft:" + bare);
        return id == null ? null : id.toString();
    }

    private static boolean occupiedByOther(Level level, BlockPos target, EntityMaid maid) {
        for (Entity entity : level.getEntities(maid, new AABB(target))) {
            if (entity.isAlive() && !entity.isSpectator()) {
                return true;
            }
        }
        return false;
    }

    private static JsonObject accepted(
            EntityMaid maid, BlockPos target, String blockId, String itemId, String name,
            boolean looked, Float yaw, String replaced) {
        JsonObject data = base();
        data.addProperty("accepted", true);
        data.addProperty("async", true);
        data.addProperty("looked", looked);
        data.addProperty("addressed", !looked);
        data.addProperty("at", offsetOf(maid, target));
        data.addProperty("item", itemId);
        data.addProperty("block", blockId);
        data.addProperty("name", name);
        data.addProperty("count", 1);
        data.addProperty("facing", yaw == null ? "" : facingOf(yaw));
        data.addProperty("replaced_action", replaced);
        return Envelope.response(Protocol.MAID_PLACE_RESULT, data);
    }

    private static JsonObject base() {
        JsonObject data = new JsonObject();
        data.addProperty("reason_code", "");
        data.addProperty("reason", "");
        return data;
    }

    private static JsonObject rejected(
            String reasonCode, String reason, String item, String at, String facing,
            JsonObject scene) {
        JsonObject data = base();
        data.addProperty("accepted", false);
        data.addProperty("async", false);
        data.addProperty("looked", at.isEmpty());
        data.addProperty("addressed", !at.isEmpty());
        data.addProperty("at", at);
        data.addProperty("item", item);
        data.addProperty("block", "");
        data.addProperty("name", "");
        data.addProperty("count", 1);
        data.addProperty("facing", facing);
        data.addProperty("replaced_action", "");
        data.addProperty("reason_code", reasonCode);
        data.addProperty("reason", reason);
        if (scene != null) {
            data.add("scene", scene);
        }
        return Envelope.response(Protocol.MAID_PLACE_RESULT, data);
    }

    /** 那一格与她脚下这一格的相对关系；失败回执用它给模型改计划的料。 */
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
        BlockPos above = target.above();
        BlockState aboveState = level.getBlockState(above);
        scene.addProperty(
                "blocked_above",
                !aboveState.isAir() && !aboveState.canBeReplaced()
                        && !aboveState.getCollisionShape(level, above).isEmpty());
        scene.addProperty("stand_room", MineTargets.standRoom(maid, target));
        return scene;
    }

    private static String offsetOf(EntityMaid maid, BlockPos target) {
        BlockPos feet = maid.blockPosition();
        return MineRules.formatOffset(
                target.getX() - feet.getX(), target.getY() - feet.getY(), target.getZ() - feet.getZ());
    }

    private static String itemId(Item item) {
        return BuiltInRegistries.ITEM.getKey(item).toString();
    }

    private static String facingOf(float yaw) {
        Direction direction = Direction.fromYRot(yaw);
        return direction.getName();
    }
}
