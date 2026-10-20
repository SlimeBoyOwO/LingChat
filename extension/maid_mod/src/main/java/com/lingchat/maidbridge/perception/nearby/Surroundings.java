package com.lingchat.maidbridge.perception.nearby;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.Protocol;
import com.lingchat.maidbridge.perception.EventFrames;
import com.lingchat.maidbridge.perception.world.PerceptionSnapshot;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

/**
 * 女仆身边的这一小片世界：她正对着什么、脚下踩着哪、身旁有谁、主人离她多远。
 *
 * <p>与 {@link PerceptionSnapshot} 一样是刻意做成不认识 Minecraft 的纯数据：读世界在
 * {@link SurroundingsReader}，判定在 {@link SurroundingsDiff}，两边于是都能脱开游戏跑单测。
 * 协议里的那份外形（{@link #response}）也留在这里，与判定分在同一个包的两侧。
 *
 * <p>字段一律是注册表 id 与机器可读的枚举串，中文措辞留给插件——专用服务端上
 * {@code getName()} 只有英文（甚至裸翻译键），中文只在客户端按玩家语言选得出来，
 * 所以协议里绝不出现显示名。
 *
 * @param entityId 被监控的女仆 uuid
 * @param dimension 维度 id，如 {@code minecraft:overworld}
 * @param gaze 视线正前方第一个命中的东西（方块、实体或什么都没有）
 * @param feetBlock 脚下那格的方块 id
 * @param footing 脚下属于哪一类地，见 {@link SurroundingsDiff#groundKind}
 * @param skyAbove 她头顶能不能看到天（在洞里、屋里为假）
 * @param hasSky 这个维度有没有天空；下界与末地没有，那里不谈「露天」
 * @param light 她所在位置的光照等级 0-15
 * @param owner 主人；没有主人或不在同一维度时见 {@link Owner}
 * @param nearby 附近的活物，按距离升序、已截断，且不含主人与她自己
 * @param nearbyTruncated 附近还有没列进来的活物
 */
public record Surroundings(
        String entityId,
        String dimension,
        Gaze gaze,
        String feetBlock,
        String footing,
        boolean skyAbove,
        boolean hasSky,
        int light,
        Owner owner,
        List<Nearby> nearby,
        boolean nearbyTruncated) {

    /** 一次视线命中的结果：{@code kind} 取 {@code block} / {@code entity} / {@code none}。 */
    public record Gaze(String kind, String id, double distance) {
        public static Gaze none() {
            return new Gaze("none", "", -1.0);
        }
    }

    /**
     * 主人。没有主人、或主人在别的维度时 {@code distance} 为 -1、
     * {@code sameDimension} 为假——插件据此说「你不在他身边」，而不是编一个距离。
     */
    public record Owner(String name, double distance, boolean visible, boolean sameDimension) {
        public static Owner absent() {
            return new Owner("", -1.0, false, false);
        }
    }

    /**
     * 附近的一只活物。
     *
     * <p>{@code category} 取 {@code player} / {@code hostile} / {@code maid} / {@code living}；
     * {@code direction} 是相对她身体朝向的八向方位；{@code visible} 是她看不看得见它
     * （无遮挡，不要求朝着它）。看不见的只在 {@link Profile#heardRadius()} 内保留，
     * 插件据此说「听到…的声音」。
     */
    public record Nearby(String type, String category, double distance, String direction, boolean visible) {
    }

    /** 一次最多列几只：看得见与听到合计不超过这个数，再多就是噪音。 */
    public static final int NEARBY_LIMIT = 3;

    /**
     * 感知档：看得见的看多远、看不见的只算多近、一次最多列几个。
     *
     * <p>自动感知（周期采样、开口时顺手带的那句）用较小的看得见半径——默认跟车万女仆的
     * 工作范围走；玩家明确问「你周围有什么」时用较大的 {@code query} 档，好让她答得出
     * 十几格外的目标。两档对看不见的活物都只留 {@code heardRadius} 以内的声音。
     */
    public record Profile(double visibleRadius, double heardRadius, int limit) {
        public static Profile of(double visibleRadius, double heardRadius) {
            return new Profile(visibleRadius, heardRadius, NEARBY_LIMIT);
        }
    }

    /** 按感知档筛掉「看不见又太远」的，剩下的才是真正算数的候选。 */
    public static List<Nearby> eligible(List<Nearby> raw, Profile profile) {
        List<Nearby> kept = new ArrayList<>();
        for (Nearby one : raw) {
            double reach = one.visible() ? profile.visibleRadius() : profile.heardRadius();
            if (one.distance() <= reach) {
                kept.add(one);
            }
        }
        return kept;
    }

    /** 按距离升序并截断。读世界那一侧只管把范围内的都塞进来，筛选、排序与截断放在这里好单测。 */
    public static List<Nearby> trimmed(List<Nearby> raw) {
        return trimmed(raw, NEARBY_LIMIT);
    }

    /** 同上，上限由调用方给。 */
    public static List<Nearby> trimmed(List<Nearby> raw, int limit) {
        List<Nearby> sorted = new ArrayList<>(raw);
        sorted.sort(Comparator.comparingDouble(Nearby::distance));
        return sorted.size() <= limit ? sorted : new ArrayList<>(sorted.subList(0, limit));
    }

    /**
     * 八向方位 token。以她**身体**的朝向为「前」，参数是目标相对她的水平偏移。
     *
     * <p>Minecraft 的 yaw 是「0 度朝 +Z，顺时针为正」，于是目标的绝对方位是
     * {@code atan2(-dx, dz)}，减去她的 yaw 就是相对她的角度；正的角度在她右手边。
     *
     * <p>这里用身体朝向而不是头部朝向：她不一定是面朝哪就看哪，而「她周围有什么」
     * 说的是她站着的方位。视线那条用的是另一套（{@code getViewVector}，头部朝向）。
     */
    public static String direction(double yaw, double dx, double dz) {
        if (dx == 0.0 && dz == 0.0) {
            return "front";
        }
        double bearing = Math.toDegrees(Math.atan2(-dx, dz));
        double relative = ((bearing - yaw + 180.0) % 360.0 + 360.0) % 360.0 - 180.0;
        double magnitude = Math.abs(relative);
        String side = relative >= 0 ? "right" : "left";
        if (magnitude <= 22.5) {
            return "front";
        }
        if (magnitude <= 67.5) {
            return "front_" + side;
        }
        if (magnitude <= 112.5) {
            return side;
        }
        if (magnitude <= 157.5) {
            return "back_" + side;
        }
        return "back";
    }

    /** 按需查询的响应帧：插件拿到它翻成一段中文事实句。 */
    public static JsonObject response(Surroundings s) {
        return response(s, List.of());
    }

    /** 同上，另带一份附近能挖的方块 id（只有按需查询要，周期采样不带）。 */
    public static JsonObject response(Surroundings s, List<String> blocks) {
        JsonObject data = new JsonObject();
        data.addProperty("maid_id", s.entityId());
        data.addProperty("dimension", s.dimension());

        JsonObject gaze = new JsonObject();
        gaze.addProperty("kind", s.gaze().kind());
        gaze.addProperty("id", s.gaze().id());
        gaze.addProperty("distance", EventFrames.round(s.gaze().distance()));
        data.add("looking_at", gaze);

        JsonObject standing = new JsonObject();
        standing.addProperty("id", s.feetBlock());
        standing.addProperty("footing", s.footing());
        data.add("standing_on", standing);
        data.addProperty("sheltered", !s.skyAbove());
        data.addProperty("has_sky", s.hasSky());
        data.addProperty("light", s.light());

        JsonObject owner = new JsonObject();
        owner.addProperty("name", s.owner().name());
        owner.addProperty("distance", EventFrames.round(s.owner().distance()));
        owner.addProperty("visible", s.owner().visible());
        owner.addProperty("same_dimension", s.owner().sameDimension());
        data.add("owner", owner);

        JsonArray nearby = new JsonArray();
        for (Nearby one : s.nearby()) {
            JsonObject item = new JsonObject();
            item.addProperty("type", one.type());
            item.addProperty("category", one.category());
            item.addProperty("distance", EventFrames.round(one.distance()));
            item.addProperty("direction", one.direction());
            item.addProperty("visible", one.visible());
            nearby.add(item);
        }
        data.add("nearby", nearby);
        data.addProperty("truncated", s.nearbyTruncated());

        // 附近能挖的方块：只给注册表 id，中文名与「拿它去挖」的判断留给插件和模型
        JsonArray blockList = new JsonArray();
        blocks.forEach(blockList::add);
        data.add("blocks", blockList);
        return Envelope.response(Protocol.MAID_LOOK_AROUND_RESULT, data);
    }
}
