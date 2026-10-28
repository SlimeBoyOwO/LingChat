package com.lingchat.maidbridge.action.attack;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.github.tartaricacid.touhoulittlemaid.util.TaskEquipUtil;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.action.core.MaidActionController;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.action.mine.MaidMineController;
import com.lingchat.maidbridge.action.place.MaidPlaceController;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.bridge.Protocol;
import com.lingchat.maidbridge.maid.MaidCondition;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.EventFrames;
import com.lingchat.maidbridge.perception.nearby.CreatureMemory;
import com.lingchat.maidbridge.perception.nearby.SurroundingsReader;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.function.Predicate;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.MobCategory;
import net.minecraft.world.entity.ai.attributes.Attributes;
import net.minecraft.world.entity.decoration.ArmorStand;
import net.minecraft.world.entity.monster.Enemy;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.phys.AABB;

/**
 * {@code maid_attack}：让她去打指定的活物，可以一次点好几只。
 *
 * <p>目标由插件翻成 {@code owner} / {@code other_player} / {@code hostile} / {@code animal}、
 * 一个实体注册表 id，或者另一个玩家的名字，这里找出离她最近的几只。<b>不走车万女仆的
 * {@code canAttack} 白名单</b>：主人自己也算目标，村民、动物、别的玩家都能点名——这是这个
 * 工具存在的意义，该不该打由玩家决定。
 *
 * <p>受理之后就交给 {@link MaidAttackController} 每 tick 推进：追过去、贴身、按攻击速度
 * 一下一下地打。不依赖她的日程表、工作模式与车万女仆自己的攻击 AI，玩家半夜喊一句也能打。
 *
 * <p>附近已经没有这种活物时，先问她记得的活物（{@link CreatureMemory}）：刚被她或玩家
 * 打死的、已经走远的，都说得清，比一句「附近没有」有用得多。
 */
public class MaidAttackHandler implements MessageHandler {
    /** 找目标看这么远：用来区分「附近没有这样的活物」与「有，但太远」。 */
    private static final double FIND_RADIUS = 32.0;
    /** 一次最多点几只。再多她也顾不过来，插件那边也按同一个上限截。 */
    private static final int MAX_TARGETS = 8;

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
        if (MaidCondition.isUnconscious(maid)) {
            return rejected("maid_unconscious", "她昏迷不醒，叫不动", "");
        }
        if (MaidQueries.carriedByPlayer(maid)) {
            return rejected("maid_carried", "玩家正抱着她，先放下来再让她动手", "");
        }
        return attack(server, maid, Envelope.data(request));
    }

    private static JsonObject attack(MinecraftServer server, EntityMaid maid, JsonObject data) {
        String rawTarget = Envelope.string(data, "target");
        String token = rawTarget.trim().toLowerCase(Locale.ROOT);
        if (token.isEmpty()) {
            return rejected("missing_target", "没有说让她打谁", "");
        }
        // 没给或认不出就按「追着打」：这是默认语义，轻轻一下得玩家明说
        boolean singleHit = "light".equalsIgnoreCase(Envelope.string(data, "style"));
        int count = parseCount(data);
        List<LivingEntity> found = findTargets(maid, token);
        List<LivingEntity> targets = new ArrayList<>();
        for (LivingEntity one : found) {
            if (targets.size() >= count) {
                break;
            }
            if (withinReach(maid, one)) {
                targets.add(one);
            }
        }
        if (targets.isEmpty()) {
            if (!found.isEmpty()) {
                return rejected("target_too_far", "那个目标在她居家活动范围之外，先走近一点再让她打", token);
            }
            return missedTarget(maid, token);
        }
        // 已经在追打同一个目标：只回执，不重启。模型常把「还在打」读成「需要再叫一次」，
        // 重复调用会不断取消并重开这一仗；回执里明说不用再叫。
        if (!singleHit && MaidAttackController.isFighting(maid, targets.get(0))) {
            return result(maid, List.of(targets.get(0)), false, false, true, count);
        }

        // 插件发起的动作每 tick 写寻路目标，与追打会互相顶
        MaidActionController.cancelCurrent(server);
        MaidMineController.cancel(server);
        MaidPlaceController.cancel(server);
        MaidActuator.standUp(maid);
        // 轻轻一下不用换武器：那一下的伤害是固定的，换刀反而像是要动真格
        boolean weapon = singleHit ? isWeapon(maid.getMainHandItem()) : equipWeapon(maid);
        MaidAttackController.start(maid, targets, singleHit);

        return result(maid, targets, weapon, singleHit, false, count);
    }

    /** 受理回执；{@code alreadyFighting} 表示这一仗已经在打，没有重启。 */
    private static JsonObject result(
            EntityMaid maid, List<LivingEntity> targets, boolean weapon, boolean singleHit,
            boolean alreadyFighting, int requested) {
        JsonObject data = base();
        data.addProperty("accepted", true);
        data.addProperty("weapon", weapon);
        data.addProperty("style", singleHit ? "light" : "fight");
        data.addProperty("already_fighting", alreadyFighting);
        data.addProperty("requested", requested);
        JsonArray list = new JsonArray();
        for (LivingEntity target : targets) {
            list.add(targetEntry(maid, target));
        }
        data.add("targets", list);
        // 老插件只读这一个：第一只就是她马上要去打的那只
        data.add("target", targetEntry(maid, targets.get(0)));
        return Envelope.response(Protocol.MAID_ATTACK_RESULT, data);
    }

    private static JsonObject targetEntry(EntityMaid maid, LivingEntity target) {
        JsonObject entry = new JsonObject();
        entry.addProperty("id", target.getStringUUID());
        entry.addProperty("type", BuiltInRegistries.ENTITY_TYPE.getKey(target.getType()).toString());
        entry.addProperty("category", SurroundingsReader.categoryOf(target));
        entry.addProperty("name", target.getName().getString());
        entry.addProperty("distance", EventFrames.round(Math.sqrt(maid.distanceToSqr(target))));
        entry.addProperty("owner", target == maid.getOwner());
        return entry;
    }

    /**
     * 附近找不到目标：问她记得的活物。
     *
     * <p>目标可能刚被谁打死、或者已经不在附近了。说得清就照实说，别一句「附近没有这样的
     * 活物」把模型推回原样重试：它需要的是改计划，不是再叫一次。
     */
    private static JsonObject missedTarget(EntityMaid maid, String token) {
        CreatureMemory.Known known = CreatureMemory.take(knownMatcher(token), maid.level().getGameTime());
        if (known == null) {
            ServerPlayer far = namedPlayerOutOfRange(maid, token);
            if (far != null) {
                return rejected("target_too_far", far.getName().getString() + "离她太远，先走近一点再让她打", token);
            }
            return rejected("no_target", "她附近没有这样的活物", token);
        }
        if (!known.dead()) {
            return rejected("target_gone", "那个目标刚才还在附近，现在已经不在了", token);
        }
        return switch (known.killedBy()) {
            case "maid" -> rejected("target_killed_by_maid", "那个目标已经被她自己打倒了", token);
            case "owner" -> rejected("target_killed_by_owner", "那个目标已经被主人打死了", token);
            case "other" -> rejected("target_killed_by_other", "那个目标已经被别的生物打死了", token);
            default -> rejected("target_dead", "那个目标已经死了", token);
        };
    }

    /** 记忆里的活物按哪种写法认：与找活物那套同一个口径。 */
    private static Predicate<CreatureMemory.Known> knownMatcher(String token) {
        if ("hostile".equals(token)) {
            return known -> "hostile".equals(known.category());
        }
        if ("animal".equals(token)) {
            return CreatureMemory.Known::animal;
        }
        if ("other_player".equals(token) || "player".equals(token)) {
            return known -> "player".equals(known.category());
        }
        ResourceLocation id = ResourceLocation.tryParse(token.contains(":") ? token : "minecraft:" + token);
        return known -> (id != null && id.toString().equals(known.type()))
                || (!known.name().isEmpty() && known.name().equalsIgnoreCase(token));
    }

    private static int parseCount(JsonObject data) {
        try {
            return Math.max(1, Math.min(MAX_TARGETS, data.get("count").getAsInt()));
        } catch (RuntimeException missingOrNotANumber) {
            return 1;
        }
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

    /** 按她附近找符合条件的活物，最近的排前面。找不到给空表。 */
    private static List<LivingEntity> findTargets(EntityMaid maid, String token) {
        if ("owner".equals(token)) {
            return maid.getOwner() instanceof LivingEntity owner ? List.of(owner) : List.of();
        }
        ServerPlayer owner = MaidQueries.owner(maid);
        Predicate<LivingEntity> matches = matcher(token, owner);
        AABB area = maid.getBoundingBox().inflate(FIND_RADIUS, 8.0, FIND_RADIUS);
        return maid.level()
                .getEntitiesOfClass(LivingEntity.class, area,
                        entity -> entity != maid && entity.isAlive() && !entity.isSpectator()
                                && !(entity instanceof ArmorStand) && matches.test(entity))
                .stream()
                .sorted(Comparator.comparingDouble(maid::distanceToSqr))
                .toList();
    }

    /**
     * 点名的是谁。
     *
     * <p>{@code other_player} 是「主人以外最近的那个玩家」；此外的写法先按实体注册表 id 认
     * （插件把中文名翻成 id、英文短名补成 {@code minecraft:短名}），再按玩家名认——另一个
     * 玩家在感知里只有名字，模型能照着名字叫出「打 Layeray」。
     */
    private static Predicate<LivingEntity> matcher(String token, ServerPlayer owner) {
        if ("hostile".equals(token)) {
            // 与身边那一圈用同一个口径：史莱姆、恶魂这些也敌视玩家，而车万女仆自己不是
            return entity -> entity instanceof Enemy;
        }
        if ("animal".equals(token)) {
            return entity -> entity.getType().getCategory() == MobCategory.CREATURE;
        }
        if ("other_player".equals(token) || "player".equals(token)) {
            return entity -> entity instanceof ServerPlayer && entity != owner;
        }
        ResourceLocation id = ResourceLocation.tryParse(token.contains(":") ? token : "minecraft:" + token);
        return entity -> (id != null && id.equals(BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType())))
                || (entity instanceof ServerPlayer player
                        && player != owner
                        && player.getName().getString().equalsIgnoreCase(token));
    }

    /** 同名玩家就在这个世界里、只是离她太远时，说清是距离问题而不是没有这个人。 */
    private static ServerPlayer namedPlayerOutOfRange(EntityMaid maid, String token) {
        for (Player player : maid.level().players()) {
            if (player instanceof ServerPlayer other
                    && other != maid.getOwner()
                    && other.getName().getString().equalsIgnoreCase(token)) {
                return other;
            }
        }
        return null;
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
