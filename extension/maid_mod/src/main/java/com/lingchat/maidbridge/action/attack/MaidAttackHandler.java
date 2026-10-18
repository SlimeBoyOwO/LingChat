package com.lingchat.maidbridge.action.attack;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.github.tartaricacid.touhoulittlemaid.util.TaskEquipUtil;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.action.core.MaidActionController;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.bridge.Protocol;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.action.mine.MaidMineController;
import com.lingchat.maidbridge.perception.EventFrames;
import java.util.Comparator;
import java.util.Locale;
import java.util.function.Predicate;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.MobCategory;
import net.minecraft.world.entity.ai.attributes.Attributes;
import net.minecraft.world.entity.decoration.ArmorStand;
import net.minecraft.world.entity.monster.Enemy;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.phys.AABB;

/**
 * {@code maid_attack}：让她去打指定的那个活物。
 *
 * <p>目标由插件翻成 {@code owner} / {@code hostile} / {@code animal} 或一个实体注册表 id，
 * 这里找离她最近的那一个。<b>不走车万女仆的 {@code canAttack} 白名单</b>：主人自己也算目标，
 * 村民、动物、别的玩家都能点名——这是这个工具存在的意义，该不该打由玩家决定。
 *
 * <p>受理之后就交给 {@link MaidAttackController} 每 tick 推进：追过去、贴身、按攻击速度
 * 一下一下地打。不依赖她的日程表、工作模式与车万女仆自己的攻击 AI，玩家半夜喊一句也能打。
 */
public class MaidAttackHandler implements MessageHandler {
    /** 找目标看这么远：用来区分「附近没有这样的活物」与「有，但太远」。 */
    private static final double FIND_RADIUS = 32.0;

    private final MinecraftServer server;

    public MaidAttackHandler(MinecraftServer server) {
        this.server = server;
    }

    @Override
    public JsonObject handle(JsonObject request) {
        if (server == null) {
            return Envelope.error(MaidQueries.NOT_READY);
        }
        EntityMaid maid = MaidQueries.findMaidById(server, Envelope.string(Envelope.data(request), "maid_id"));
        if (maid == null) {
            return rejected("maid_not_found", MaidQueries.NOT_FOUND, "");
        }
        if (MaidQueries.carriedByPlayer(maid)) {
            return rejected("maid_carried", "玩家正抱着她，先放下来再让她动手", "");
        }
        JsonObject data = Envelope.data(request);
        return attack(server, maid, Envelope.string(data, "target"), Envelope.string(data, "style"));
    }

    private static JsonObject attack(MinecraftServer server, EntityMaid maid, String rawTarget, String rawStyle) {
        String token = rawTarget == null ? "" : rawTarget.trim().toLowerCase(Locale.ROOT);
        if (token.isEmpty()) {
            return rejected("missing_target", "没有说让她打谁", "");
        }
        // 没给或认不出就按「追着打」：这是默认语义，轻轻一下得玩家明说
        boolean singleHit = "light".equalsIgnoreCase(rawStyle == null ? "" : rawStyle.trim());
        LivingEntity target = findTarget(maid, token);
        if (target == null) {
            return rejected("no_target", "她附近没有这样的活物", token);
        }
        double distance = Math.sqrt(maid.distanceToSqr(target));
        // 居家模式下活动范围是玩家自己划的，范围外的目标追不到；跟随模式不用再看主人
        // 离目标多远——追打期间 MaidFollowOwnerTask 的传送被 LongTaskGuard 拦住了。
        if (!withinReach(maid, target)) {
            return rejected("target_too_far", "那个目标在她居家活动范围之外，先走近一点再让她打", token);
        }

        // 插件发起的动作每 tick 写寻路目标，与追打会互相顶
        MaidActionController.cancelCurrent(server);
        MaidMineController.cancel(server);
        MaidActuator.standUp(maid);
        // 轻轻一下不用换武器：那一下的伤害是固定的，换刀反而像是要动真格
        boolean weapon = singleHit ? isWeapon(maid.getMainHandItem()) : equipWeapon(maid);
        MaidAttackController.start(maid, target, singleHit);

        JsonObject data = base();
        data.addProperty("accepted", true);
        data.addProperty("weapon", weapon);
        data.addProperty("style", singleHit ? "light" : "fight");
        JsonObject entry = new JsonObject();
        entry.addProperty("id", target.getStringUUID());
        entry.addProperty("type", BuiltInRegistries.ENTITY_TYPE.getKey(target.getType()).toString());
        entry.addProperty("name", target.getName().getString());
        entry.addProperty("distance", EventFrames.round(distance));
        entry.addProperty("owner", target == maid.getOwner());
        data.add("target", entry);
        return Envelope.response(Protocol.MAID_ATTACK_RESULT, data);
    }

    /**
     * 把背包里带攻击力的东西换到手上；没有就空手打（1 点伤害）。
     *
     * <p>和车万女仆自己的攻击模式同一个口径：只认带 {@code ATTACK_DAMAGE} 的武器，
     * 所以不会为了打架把她的锄头换走。
     */
    private static boolean equipWeapon(EntityMaid maid) {
        TaskEquipUtil.tryEquipFromBackpack(maid, MaidAttackHandler::isWeapon);
        return isWeapon(maid.getMainHandItem());
    }

    private static boolean isWeapon(ItemStack stack) {
        return stack.getAttributeModifiers().modifiers().stream()
                .anyMatch(modifier -> modifier.attribute().is(Attributes.ATTACK_DAMAGE));
    }

    /**
     * 这个目标她追得到吗。
     *
     * <p>居家模式看活动范围（车万女仆每 tick 会清掉范围之外的寻路目标，走出去追是白追）。
     * 跟随模式不再看「主人到目标」的距离：追打期间 {@code MaidFollowOwnerTask} 的传送被
     * {@code LongTaskGuard} 拦住了，她能一直追到搜索半径的边缘；打完收工后跟随行为恢复。
     */
    private static boolean withinReach(EntityMaid maid, LivingEntity target) {
        if (maid.isHomeModeEnable()) {
            return maid.isWithinRestriction(target.blockPosition());
        }
        return true;
    }

    /** 按她附近找最近的、符合条件的活物。找不到给 null。 */
    private static LivingEntity findTarget(EntityMaid maid, String token) {
        if ("owner".equals(token)) {
            return maid.getOwner() instanceof LivingEntity owner ? owner : null;
        }
        Predicate<LivingEntity> matches = matcher(token);
        AABB area = maid.getBoundingBox().inflate(FIND_RADIUS, 8.0, FIND_RADIUS);
        return maid.level()
                .getEntitiesOfClass(LivingEntity.class, area,
                        entity -> entity != maid && entity.isAlive() && !entity.isSpectator()
                                && !(entity instanceof ArmorStand) && matches.test(entity))
                .stream()
                .min(Comparator.comparingDouble(maid::distanceToSqr))
                .orElse(null);
    }

    private static Predicate<LivingEntity> matcher(String token) {
        if ("hostile".equals(token)) {
            // 与身边那一圈用同一个口径：史莱姆、恶魂这些也敌视玩家，而车万女仆自己不是
            return entity -> entity instanceof Enemy;
        }
        if ("animal".equals(token)) {
            return entity -> entity.getType().getCategory() == MobCategory.CREATURE;
        }
        // 只在写全了命名空间时按 id 认：短名归插件翻（它知道中文名与英文短名）
        ResourceLocation id = token.contains(":") ? ResourceLocation.tryParse(token) : null;
        if (id == null) {
            return entity -> false;
        }
        return entity -> id.equals(BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()));
    }

    private static JsonObject base() {
        JsonObject data = new JsonObject();
        data.addProperty("reason_code", "");
        data.addProperty("reason", "");
        return data;
    }

    private static JsonObject rejected(String reasonCode, String reason, String target) {
        JsonObject data = base();
        data.addProperty("accepted", false);
        data.addProperty("weapon", false);
        data.addProperty("target_token", target == null ? "" : target);
        data.addProperty("reason_code", reasonCode);
        data.addProperty("reason", reason);
        return Envelope.response(Protocol.MAID_ATTACK_RESULT, data);
    }
}
