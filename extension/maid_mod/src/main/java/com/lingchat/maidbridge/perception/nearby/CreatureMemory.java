package com.lingchat.maidbridge.perception.nearby;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.function.Predicate;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.MobCategory;

/**
 * 她记得的活物：最近看见过谁、现在还在不在、死在谁手里。
 *
 * <p>快照只说得清此刻。玩家说「打那只僵尸」，她还没走到，僵尸先被玩家射死了；或者
 * 她追到一半被抱了起来——这时候再去问世界只能得到「找不到」，要回答「它怎么了」，
 * 只能靠这份短期记忆。
 *
 * <p>喂给它的只有两处：采样时读到的活物，以及她身边死掉的那些（见 {@code DeathReporter}）。
 * 一条从最后一次看见算起留两分钟；被查询用掉就删，下次看见再记一条——记忆说的是过去
 * 感知过什么，不是一份永远正确的世界账本。
 */
public final class CreatureMemory {
    /** 一条记录留多久（tick）。两分钟够覆盖「看见 → 下令 → 发现它没了」这一来一回。 */
    public static final int REMEMBER_TICKS = 2400;
    /** 最多记这么多只：只留最近的，免得一场围城把内存撑满。 */
    private static final int MAX_ENTRIES = 16;

    /**
     * 一条记得的活物。
     *
     * <p>{@code killedBy} 取 {@code maid} / {@code owner} / {@code other}；死因是环境
     * （摔、烧、淹）时为空串。{@code animal} 记的是原版生物分类，攻击指令里的
     * {@code animal} 要按它认。
     */
    public record Known(
            String id,
            String type,
            String category,
            String name,
            boolean animal,
            double distance,
            long seenTick,
            boolean dead,
            String killedBy,
            String killerType) {
    }

    private static final class Entry {
        final String id;
        final ServerLevel level;
        String type;
        String category;
        String name;
        boolean animal;
        double distance;
        long seenTick;
        boolean dead;
        String killedBy = "";
        String killerType = "";

        Entry(LivingEntity entity, EntityMaid maid, long tick) {
            this.id = entity.getStringUUID();
            this.level = (ServerLevel) maid.level();
            this.type = SurroundingsReader.typeId(entity);
            this.category = SurroundingsReader.categoryOf(entity);
            this.name = entity instanceof ServerPlayer player ? player.getName().getString() : "";
            this.animal = entity.getType().getCategory() == MobCategory.CREATURE;
            this.distance = Math.sqrt(maid.distanceToSqr(entity));
            this.seenTick = tick;
        }
    }

    private static final Map<String, Entry> entries = new HashMap<>();

    private CreatureMemory() {
    }

    /** 看见了她：刷新它的记录。周期采样与按需查询都从这里记。 */
    public static void observe(LivingEntity entity, EntityMaid maid, long tick) {
        if (entity == null || maid == null || entity == maid || !(maid.level() instanceof ServerLevel)) {
            return;
        }
        prune(tick);
        Entry entry = entries.get(entity.getStringUUID());
        if (entry == null) {
            if (entries.size() >= MAX_ENTRIES) {
                evictOldest();
            }
            entries.put(entity.getStringUUID(), new Entry(entity, maid, tick));
            return;
        }
        entry.distance = Math.sqrt(maid.distanceToSqr(entity));
        entry.seenTick = tick;
    }

    /** 她身边死了一只：记下死在谁手里。没感知过的不管，快照本来也不说它。 */
    public static void markDead(LivingEntity dead, Entity attacker, EntityMaid maid, long tick) {
        Entry entry = entries.get(dead.getStringUUID());
        if (entry == null || entry.dead || maid == null) {
            return;
        }
        entry.dead = true;
        entry.seenTick = tick;
        entry.distance = Math.sqrt(maid.distanceToSqr(dead));
        entry.killedBy = killerOf(attacker, maid);
        entry.killerType = attacker == null ? "" : SurroundingsReader.typeId(attacker);
    }

    /**
     * 最近记得的、符合条件的那一只，取走后删掉。
     *
     * <p>用来解释「为什么打不了」：她身边已经没有这样的活物了，但刚刚还记得一只。
     * 同一条记忆只够回答一次，之后再问就该重新看了。
     */
    public static Known take(Predicate<Known> match, long now) {
        prune(now);
        Entry best = null;
        for (Entry entry : entries.values()) {
            if (match.test(snapshot(entry)) && (best == null || entry.seenTick > best.seenTick)) {
                best = entry;
            }
        }
        if (best == null) {
            return null;
        }
        entries.remove(best.id);
        return snapshot(best);
    }

    /** 按 uuid 找一条，不删：追打途中要拿它的类型与死因写失败那条旁白。 */
    public static Known lookup(String id, long now) {
        prune(now);
        Entry entry = entries.get(id);
        return entry == null ? null : snapshot(entry);
    }

    /**
     * 最近记得的几只，给「看一眼周围」补一句「刚才见过、现在不在的」。
     *
     * <p>还活着、此刻仍在她身边的不列：它们本来就出现在附近那份清单里。
     */
    public static List<Known> recent(EntityMaid maid, long now, double radius, int limit) {
        prune(now);
        List<Entry> kept = new ArrayList<>();
        for (Entry entry : entries.values()) {
            if (!entry.dead && entry.level == maid.level()) {
                Entity entity = entry.level.getEntity(UUID.fromString(entry.id));
                if (entity != null && entity.isAlive() && maid.distanceToSqr(entity) <= radius * radius) {
                    continue;
                }
            }
            kept.add(entry);
        }
        kept.sort(Comparator.comparingLong((Entry entry) -> entry.seenTick).reversed());
        List<Known> result = new ArrayList<>();
        for (Entry entry : kept) {
            if (result.size() >= limit) {
                break;
            }
            result.add(snapshot(entry));
        }
        return result;
    }

    /** 停服、换存档时清干净。 */
    public static void clear() {
        entries.clear();
    }

    /** 死因归到谁：她自己、主人、别的生物或环境。 */
    private static String killerOf(Entity attacker, EntityMaid maid) {
        if (attacker == null) {
            return "";
        }
        if (attacker == maid) {
            return "maid";
        }
        return attacker == maid.getOwner() && maid.getOwner() != null ? "owner" : "other";
    }

    private static Known snapshot(Entry entry) {
        return new Known(
                entry.id,
                entry.type,
                entry.category,
                entry.name,
                entry.animal,
                entry.distance,
                entry.seenTick,
                entry.dead,
                entry.killedBy,
                entry.killerType);
    }

    private static void prune(long now) {
        entries.values().removeIf(entry -> now - entry.seenTick > REMEMBER_TICKS);
    }

    private static void evictOldest() {
        entries.values().stream()
                .min(Comparator.comparingLong(entry -> entry.seenTick))
                .ifPresent(oldest -> entries.remove(oldest.id));
    }
}
