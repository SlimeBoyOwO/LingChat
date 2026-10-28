package com.lingchat.maidbridge.perception.nearby;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.Protocol;
import com.lingchat.maidbridge.perception.EventFrames;
import com.lingchat.maidbridge.perception.spatial.BlockSight;
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
 * {@code getName()} 只有英文（甚至裸翻译键），中文只在客户端按玩家语言选得出来。
 * 唯一的显示名是玩家自己的名字（{@link Nearby#name()}）：那是玩家取的名字、不随语言变，
 * 而另一个玩家在游戏里除此之外没有别的称呼。
 *
 * @param entityId 被监控的女仆 uuid
 * @param dimension 维度 id，如 {@code minecraft:overworld}
 * @param gaze 视线正前方第一个命中的东西（方块、实体或什么都没有）
 * @param feetBlock 脚下那格的方块 id
 * @param footing 脚下属于哪一类地，见 {@link SurroundingsDiff#groundKind}
 * @param skyAbove 她头顶能不能看到天（在洞里、屋里为假）
 * @param hasSky 这个维度有没有天空；下界与末地没有，那里不谈「露天」
 * @param overhead 头顶这一层是三档里的哪一档，见 {@link Overhead}
 * @param light 她所在位置的光照等级 0-15
 * @param owner 主人；没有主人或不在同一维度时见 {@link Owner}
 * @param nearby 附近的活物，按距离升序、已截断，且不含主人与她自己
 * @param nearbyTruncated 附近还有没列进来的活物
 * @param vehicle 她正坐在里面的载具（船、矿车这类）的实体 id；没坐载具时为空串
 * @param structure 她所在结构（村庄、矿井这类）的 id；不在结构里时为空串
 * @param posture 她此刻的姿势：{@code standing} / {@code sitting} / {@code sleeping} / {@code lying}
 *                / {@code unconscious}
 * @param liquid 她自己这一格里的液体 id（水、岩浆或模组液体）；没泡在液体里时为空串
 * @param submerged 头也浸在同一种液体里，也就是在水下游泳
 */
public record Surroundings(
        String entityId,
        String dimension,
        Gaze gaze,
        String feetBlock,
        String footing,
        boolean skyAbove,
        boolean hasSky,
        Overhead overhead,
        int light,
        Owner owner,
        List<Nearby> nearby,
        boolean nearbyTruncated,
        String vehicle,
        String structure,
        String posture,
        String liquid,
        boolean submerged) {

    /** 一次视线命中的结果：{@code kind} 取 {@code block} / {@code entity} / {@code none}。 */
    public record Gaze(String kind, String id, double distance) {
        public static Gaze none() {
            return new Gaze("none", "", -1.0);
        }
    }

    /**
     * 头顶这一层：{@code kind} 取 {@code open}（露天）/ {@code partial}（树荫、屋檐、洞口）
     * / {@code enclosed}（房间、洞穴）；没有天空的维度为空串，整项不谈。
     *
     * <p>{@code cover} 是头顶第一块挡路的方块 id，认不出材质时为空串；{@code band} 只说它
     * 离她头顶是 {@code low}（压得低）还是 {@code high}，不报具体格数。
     */
    public record Overhead(String kind, String cover, String band) {
        public static Overhead none() {
            return new Overhead("", "", "");
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
     *
     * <p>{@code name} 只有玩家才有（别的活物的显示名是翻译键，专用服务端上取不出中文）。
     * 另一个玩家在游戏里只叫得出这个名字，攻击、称呼都得靠它。
     */
    public record Nearby(
            String type, String category, double distance, String direction, boolean visible, String name) {
    }

    /** 一次最多列几只：看得见与听到合计不超过这个数，再多就是噪音。 */
    public static final int NEARBY_LIMIT = 3;

    /**
     * 感知档：看得见的看多远、看不见的只算多近、一次最多列几个。
     *
     * <p>自动感知（周期采样；出事那条旁白里顺手补一份也用它）用较小的看得见半径——默认跟车万女仆的
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

    /**
     * 同上，另带一份附近能挖的方块（只有按需查询要，周期采样不带）。
     *
     * <p>{@code blocks} 仍是 id 清单，老插件照读；{@code block_sight} 与它一一对应，
     * 说清这一种是此刻看见的（{@code now}）还是刚才看见、现在不在视野里的（{@code before}）。
     */
    public static JsonObject response(Surroundings s, List<BlockSight> blocks) {
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
        data.addProperty("overhead", s.overhead().kind());
        data.addProperty("overhead_cover", s.overhead().cover());
        data.addProperty("overhead_band", s.overhead().band());
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
            item.addProperty("name", one.name());
            nearby.add(item);
        }
        data.add("nearby", nearby);
        data.addProperty("truncated", s.nearbyTruncated());
        // 坐在船/矿车里时脚下没有「地」可说，插件改说载具；结构同理
        data.addProperty("vehicle", s.vehicle());
        data.addProperty("structure", s.structure());
        // 站着是默认，插件不念；坐着、睡着、躺在地上要说一句，否则模型会当她站着
        data.addProperty("posture", s.posture());

        // 附近能挖的方块：只给注册表 id，中文名与「拿它去挖」的判断留给插件和模型
        JsonArray blockList = new JsonArray();
        JsonArray blockSight = new JsonArray();
        for (BlockSight one : blocks) {
            blockList.add(one.id());
            JsonObject sight = new JsonObject();
            sight.addProperty("id", one.id());
            sight.addProperty("seen", one.seenNow() ? "now" : "before");
            blockSight.add(sight);
        }
        data.add("blocks", blockList);
        data.add("block_sight", blockSight);
        return Envelope.response(Protocol.MAID_LOOK_AROUND_RESULT, data);
    }
}
