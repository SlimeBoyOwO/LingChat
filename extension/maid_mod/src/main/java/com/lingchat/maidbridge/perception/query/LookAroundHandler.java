package com.lingchat.maidbridge.perception.query;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.maid.MaidInventory;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.PerceptionProfiles;
import com.lingchat.maidbridge.perception.nearby.Surroundings;
import com.lingchat.maidbridge.perception.nearby.SurroundingsReader;
import net.minecraft.core.BlockPos;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;

/**
 * {@code maid_look_around}：按需取一份她身边的快照。
 *
 * <p>与周期采样的那条路互不打扰：这是「她被问到」，所以不受感知开关与采样间隔影响——
 * 玩家问了就该答得出来。采样那条是「她自发注意」，归 {@code perceptionEnabled} 管。
 *
 * <p>请求里的 {@code profile} 决定用哪一档：{@code auto} 是玩家开口时顺手带的那份
 * （看得见按工作范围、看不见只听很近的声音，不带方块与背包清单）；缺省或 {@code query}
 * 是玩家明确问「你周围有什么」（看得见 32 格，并附附近方块与她背包里的东西）。
 * 两档都只列最近 3 个活物。
 */
public class LookAroundHandler implements MessageHandler {
    /** 附近方块扫多大：水平与竖直跟 {@code maid_mine} 在身边找方块时同一口径。 */
    private static final int BLOCK_RADIUS = 24;
    private static final int BLOCK_VERTICAL = 8;
    /** 方块种类最多列几种：模型只需要一个能下手的清单，不是一份完整普查。 */
    private static final int BLOCK_LIMIT = 12;
    /** 背包清单最多列几种：给「把东西给我」这类指令一个能用的 id 清单。 */
    private static final int BACKPACK_LIMIT = 12;
    /** 看玩家准星落点多远：与挖掘的区域锚点同一口径。 */
    private static final double GAZE_RAY = 32.0;

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
        boolean auto = "auto".equalsIgnoreCase(Envelope.string(Envelope.data(request), "profile"));
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
        JsonObject response = Surroundings.response(
                snapshot,
                SurroundingsReader.blocksNearby(maid, center, BLOCK_RADIUS, BLOCK_VERTICAL, BLOCK_LIMIT));
        // 背包清单只给按需查询：模型要能看见她身上有什么，才谈得上「把那个给我」
        Envelope.data(response).add("backpack", MaidInventory.stacks(maid, BACKPACK_LIMIT));
        return response;
    }
}
