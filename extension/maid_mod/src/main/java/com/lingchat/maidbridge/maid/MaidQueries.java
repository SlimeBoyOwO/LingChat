package com.lingchat.maidbridge.maid;

import com.github.tartaricacid.touhoulittlemaid.api.task.IMaidTask;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.github.tartaricacid.touhoulittlemaid.entity.task.TaskManager;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.action.core.MaidActionController;
import com.lingchat.maidbridge.bridge.Envelope;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.Pose;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.HitResult;

/** 查女仆、查她的主人，以及把女仆序列化成 {@code maid_status} 里的字段。 */
public final class MaidQueries {
    /** 服务端还没起来。 */
    public static final String NOT_READY = "服务端尚未就绪";
    /** 女仆走远到区块卸载后就查不到了——车万女仆的实体只在已加载区块里拿得到。 */
    public static final String NOT_FOUND = "找不到女仆（可能不在已加载的区块里）";

    private MaidQueries() {
    }

    /**
     * 她是不是被玩家当乘客抱着（Carry On 这类搬运模组）。
     *
     * <p>这种状态下她还在世界里有实体，但动不了；让她下车会把搬运数据和实体状态拆开，
     * 玩家那边会表现为「黏在手上放不下来」，所以指挥类工具一律先拒绝。
     */
    public static boolean carriedByPlayer(EntityMaid maid) {
        return maid != null && maid.isPassenger() && maid.getVehicle() instanceof Player;
    }

    /**
     * 她此刻的姿势：{@code standing} / {@code sitting} / {@code sleeping} / {@code lying}
     * / {@code unconscious}。
     *
     * <p>昏迷优先于普通睡姿：昏迷时写入睡眠位置是为了闭眼，但对外说的状态是「不省人事」，
     * 不是「睡着了」。地上躺下与昏迷一样按了一个脚下的睡位（语义是闭眼与躺姿），所以
     * 判断顺序是昏迷、桥接的躺地动作、床、坐、躺。
     */
    public static String posture(EntityMaid maid) {
        if (MaidCondition.isUnconscious(maid)) {
            return "unconscious";
        }
        if (MaidActionController.isGroundLying(maid)) {
            return "lying";
        }
        if (maid.isSleeping()) {
            return "sleeping";
        }
        if (maid.isMaidInSittingPose()) {
            return "sitting";
        }
        return maid.hasPose(Pose.SLEEPING) ? "lying" : "standing";
    }

    /**
     * 请求里点名的那只女仆；服务端没就绪、或她此刻不在已加载的区块里都给 null。
     *
     * <p>四个 handler 都从这一步开始，理由与错误措辞都收在这里，免得各写一套。
     */
    public static EntityMaid requested(MinecraftServer server, JsonObject request) {
        if (server == null) {
            return null;
        }
        return findMaidById(server, Envelope.string(Envelope.data(request), "maid_id"));
    }

    /** 查不到时该说什么。服务端没就绪与真的找不到是两回事，别混成一句。 */
    public static String missingReason(MinecraftServer server) {
        return server == null ? NOT_READY : NOT_FOUND;
    }

    /** 女仆的主人；不是玩家（没主人、主人是别的生物）时给 null。 */
    public static ServerPlayer owner(EntityMaid maid) {
        return maid.getOwner() instanceof ServerPlayer player ? player : null;
    }

    /**
     * 玩家准星射线的完整命中（方块、面与位置）；没指着方块时给 null。
     *
     * <p>挖掘要的是落点那一格，放置还要知道点的是哪一面，所以两条都从这里取。
     */
    public static BlockHitResult gazeHit(ServerPlayer player, double distance) {
        if (player == null) {
            return null;
        }
        HitResult hit = player.pick(distance, 1.0F, false);
        return hit.getType() == HitResult.Type.BLOCK ? (BlockHitResult) hit : null;
    }

    /**
     * 玩家准星射线的落点，最多 distance 格；没指着方块时给 null。
     *
     * <p>挖掘拿它当区域锚点，「看一眼周围」拿它决定方块清单以哪片为中心；两边问的是
     * 同一件事，所以放在这里，不让感知去依赖挖掘的实现。
     */
    public static BlockPos gazeAt(ServerPlayer player, double distance) {
        BlockHitResult hit = gazeHit(player, distance);
        return hit == null ? null : hit.getBlockPos();
    }

    /** 只拿得到已加载的女仆；女仆走远到区块卸载后就查不到了。 */
    public static List<EntityMaid> getAllMaids(MinecraftServer server) {
        List<EntityMaid> maids = new ArrayList<>();
        if (server == null) {
            return maids;
        }
        for (ServerLevel level : server.getAllLevels()) {
            for (Entity entity : level.getAllEntities()) {
                if (entity instanceof EntityMaid maid) {
                    maids.add(maid);
                }
            }
        }
        return maids;
    }

    public static EntityMaid findMaidById(MinecraftServer server, String maidId) {
        if (server == null || maidId == null || maidId.isEmpty()) {
            return null;
        }
        // 复活出来的是新实体，而插件配置里还是倒下那一只：所有请求统一在这里换成现在这只
        String resolved = MonitoredMaidHandler.resolve(maidId);
        try {
            UUID uuid = UUID.fromString(resolved);
            for (ServerLevel level : server.getAllLevels()) {
                if (level.getEntity(uuid) instanceof EntityMaid maid) {
                    return maid;
                }
            }
        } catch (IllegalArgumentException notAUuid) {
            // 插件也可能传来名字之类的东西，退回下面的遍历匹配
        }
        for (EntityMaid maid : getAllMaids(server)) {
            if (maid.getStringUUID().equals(resolved)) {
                return maid;
            }
        }
        return null;
    }

    public static JsonObject serialize(EntityMaid maid) {
        JsonObject result = new JsonObject();
        result.addProperty("id", maid.getStringUUID());
        result.addProperty("name", maid.getName().getString());
        result.addProperty("health", maid.getHealth());
        result.addProperty("max_health", maid.getMaxHealth());

        JsonObject position = new JsonObject();
        position.addProperty("x", maid.getX());
        position.addProperty("y", maid.getY());
        position.addProperty("z", maid.getZ());
        result.add("position", position);

        result.addProperty("dimension", maid.level().dimension().location().toString());
        result.addProperty("is_sitting", maid.isMaidInSittingPose());
        result.addProperty("is_following", !maid.isHomeModeEnable());
        result.addProperty("unconscious", MaidCondition.isUnconscious(maid));
        result.addProperty(
                "on_bed", MaidCondition.isUnconscious(maid) && MaidCondition.isOnBed(maid));

        IMaidTask task = maid.getTask();
        result.addProperty("task", task == null ? "" : task.getUid().toString());

        if (maid.getOwner() != null) {
            result.addProperty("owner", maid.getOwner().getName().getString());
        }

        result.addProperty("main_hand_item", itemId(maid.getMainHandItem()));
        result.addProperty("off_hand_item", itemId(maid.getOffhandItem()));

        JsonArray availableTasks = new JsonArray();
        for (IMaidTask available : TaskManager.getNotHiddenTaskList(maid)) {
            JsonObject entry = new JsonObject();
            entry.addProperty("id", available.getUid().toString());
            entry.addProperty("name", available.getName().getString());
            availableTasks.add(entry);
        }
        result.add("available_tasks", availableTasks);
        return result;
    }

    private static String itemId(ItemStack stack) {
        return stack.isEmpty() ? "" : BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
    }
}
