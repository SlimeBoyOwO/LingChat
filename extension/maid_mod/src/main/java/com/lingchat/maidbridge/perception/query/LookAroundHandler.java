package com.lingchat.maidbridge.perception.query;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.maid.MaidCondition;
import com.lingchat.maidbridge.maid.MaidInventory;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.EventFrames;
import com.lingchat.maidbridge.perception.PerceptionProfiles;
import com.lingchat.maidbridge.perception.nearby.CreatureMemory;
import com.lingchat.maidbridge.perception.nearby.Surroundings;
import com.lingchat.maidbridge.perception.nearby.SurroundingsReader;
import com.lingchat.maidbridge.perception.spatial.BlockSight;
import com.lingchat.maidbridge.perception.spatial.LocalTerrain;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.Level;

/**
 * {@code maid_look_around}：按需取一份她身边的快照。
 *
 * <p>与周期采样的那条路互不打扰：这是「她被问到」，所以不受感知开关与采样间隔影响——
 * 玩家问了就该答得出来。采样那条是「她自发注意」，归 {@code perceptionEnabled} 管。
 *
 * <p>请求里的 {@code profile} 决定用哪一档：{@code auto} 是周期采样用的近处那档
 * （出事那条旁白里顺手补一份也用它；看得见按工作范围、看不见只听很近的声音，不带方块与
 * 背包清单）；缺省或 {@code query} 是玩家明确问「你周围有什么」（看得见 32 格，并附附近
 * 方块与她背包里的东西）。两档都只列最近 3 个活物。
 */
public class LookAroundHandler implements MessageHandler {
    /** 附近方块扫多大：水平 24 格（挖掘找 32，清单只要看得出有哪种方块就够），竖直只从她脚下往上。 */
    private static final int BLOCK_RADIUS = 24;
    private static final int BLOCK_VERTICAL = 8;
    /** 方块种类最多列几种：模型只需要一个能下手的清单，不是一份完整普查。 */
    private static final int BLOCK_LIMIT = 12;
    /** 背包清单最多列几种：给「把东西给我」这类指令一个能用的 id 清单。 */
    private static final int BACKPACK_LIMIT = 12;
    /** 记得的活物最多列几只：只用来回答「刚才那只呢」，不是第二份活物清单。 */
    private static final int REMEMBERED_LIMIT = 5;
    /** 看玩家准星落点多远：与挖掘的区域锚点同一口径。 */
    private static final double GAZE_RAY = 32.0;
    /** 小图自己的地形快照半径：远处两格合并要看到 ±13 格。 */
    private static final int MAP_RADIUS = 14;

    private final MinecraftServer server;

    public LookAroundHandler(MinecraftServer server) {
        this.server = server;
    }

    @Override
    public JsonObject handle(JsonObject request) {
        EntityMaid maid = MaidQueries.requested(server, request);
        if (maid == null) {
            return Envelope.error(MaidQueries.missingReason(server));
        }
        if (MaidCondition.isUnconscious(maid)) {
            return Envelope.error("她昏迷不醒，感知不到周围");
        }
        JsonObject requestData = Envelope.data(request);
        boolean auto = "auto".equalsIgnoreCase(Envelope.string(requestData, "profile"));
        Surroundings snapshot = SurroundingsReader.read(maid, auto ? PerceptionProfiles.auto() : PerceptionProfiles.query());
        if (auto) {
            // 自动感知只要近处那一份：不扫方块、不列背包，省掉每次开口的几千次方块读取
            return Surroundings.response(snapshot);
        }
        // 玩家指着某处时，方块清单也跟着看那片；没指就看她自己周围。
        // 这样「看一眼周围 → 用返回的 id 去挖」和挖掘指令的区域锚点是同一个地方。
        ServerPlayer owner = MaidQueries.owner(maid);
        BlockPos gaze = owner != null && owner.level() == maid.level()
                ? MaidQueries.gazeAt(owner, GAZE_RAY)
                : null;
        BlockPos center = gaze != null ? gaze : maid.blockPosition();
        Level level = maid.level();
        LocalTerrain terrain = LocalTerrain.read(level, center, BLOCK_RADIUS, 1, BLOCK_VERTICAL);
        List<BlockSight> blocks =
                SurroundingsReader.blocksNearby(maid, center, terrain, BLOCK_LIMIT);
        boolean withMap = "map".equalsIgnoreCase(Envelope.string(requestData, "detail"));
        BlockPos mapCenter = maid.blockPosition();
        LocalTerrain mapTerrain = null;
        if (withMap) {
            // 小图总是以她为中心；玩家指着的远处那片由关系句交代，不画进图里
            mapTerrain = mapCenter.equals(center)
                    ? terrain
                    : LocalTerrain.read(level, mapCenter, MAP_RADIUS, 1, BLOCK_VERTICAL);
        }
        BlockProjection projection = BlockProjector.project(maid, terrain, mapTerrain, blocks, withMap);
        JsonObject response = Surroundings.response(snapshot, blocks);
        // 背包清单只给按需查询：模型要能看见她身上有什么，才谈得上「把那个给我」
        JsonObject body = Envelope.data(response);
        addProjection(body, projection);
        addRemembered(body, maid, level);
        body.add("backpack", MaidInventory.stacks(maid, BACKPACK_LIMIT));
        // 最后一格是头饰栏：顶在头上，不算背包存货，单独说
        ItemStack headwear = MaidInventory.headwear(maid);
        if (!headwear.isEmpty()) {
            JsonObject head = new JsonObject();
            head.addProperty("item", BuiltInRegistries.ITEM.getKey(headwear.getItem()).toString());
            head.addProperty("count", headwear.getCount());
            body.add("headwear", head);
        }
        return response;
    }

    /**
     * 她记得的活物：刚才见过、现在不在了的那几只。
     *
     * <p>只列死了或走远了的，还活着的本来就在附近那份清单里。模型问「刚才那只僵尸呢」
     * 时靠它答；也是攻击失败时能看到同一份记忆的来源。
     */
    private static void addRemembered(JsonObject body, EntityMaid maid, Level level) {
        JsonArray remembered = new JsonArray();
        for (CreatureMemory.Known one : CreatureMemory.recent(
                maid,
                level.getGameTime(),
                PerceptionProfiles.QUERY_VISIBLE_RADIUS,
                REMEMBERED_LIMIT)) {
            JsonObject item = new JsonObject();
            item.addProperty("type", one.type());
            item.addProperty("category", one.category());
            item.addProperty("name", one.name());
            item.addProperty("dead", one.dead());
            item.addProperty("killed_by", one.killedBy());
            item.addProperty("killed_by_type", one.killerType());
            item.addProperty("distance", EventFrames.round(one.distance()));
            remembered.add(item);
        }
        body.add("remembered", remembered);
    }

    private static void addProjection(JsonObject body, BlockProjection projection) {        JsonArray relations = new JsonArray();
        for (BlockRelation one : projection.relations()) {
            JsonObject item = new JsonObject();
            item.addProperty("id", one.id());
            item.addProperty("seen", one.seenNow() ? "now" : "before");
            item.addProperty("dx", one.dx());
            item.addProperty("dy", one.dy());
            item.addProperty("dz", one.dz());
            item.addProperty("facing", one.facing());
            item.addProperty("same_layer", one.sameLayer());
            item.addProperty("distance", EventFrames.round(one.distance()));
            item.addProperty("blockers", one.blockers());
            item.addProperty("blocker", one.blocker());
            item.addProperty("walk_steps", one.walkSteps());
            relations.add(item);
        }
        body.add("relations", relations);

        JsonArray clusters = new JsonArray();
        for (BlockCluster one : projection.clusters()) {
            JsonObject item = new JsonObject();
            item.addProperty("id", one.id());
            item.addProperty("cells", one.cells());
            item.addProperty("truncated", one.truncated());
            item.addProperty("dx", one.dx());
            item.addProperty("dy", one.dy());
            item.addProperty("dz", one.dz());
            item.addProperty("facing", one.facing());
            item.addProperty("distance", EventFrames.round(one.distance()));
            item.addProperty("blockers", one.blockers());
            item.addProperty("blocker", one.blocker());
            clusters.add(item);
        }
        body.add("clusters", clusters);

        if (projection.map() != null) {
            JsonObject map = new JsonObject();
            map.addProperty("radius", projection.map().radius());
            map.addProperty("text", projection.map().text());
            body.add("map", map);
        }
    }
}
