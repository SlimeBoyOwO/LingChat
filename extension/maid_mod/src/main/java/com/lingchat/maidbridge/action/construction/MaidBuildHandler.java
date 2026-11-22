package com.lingchat.maidbridge.action.construction;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.action.place.PlaceRules;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.bridge.Protocol;
import com.lingchat.maidbridge.maid.MaidCondition;
import com.lingchat.maidbridge.maid.MaidQueries;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;

/**
 * {@code maid_build}：按插件给的图纸施工。
 *
 * <p>图纸是一组相对锚点的格子，每格写清方块 id 与可选的朝向。这里只做受理前的校验：
 * 锚点与每一格都在已加载的区块里、在她活动范围内，方块认得出来而且有能放下的物品。
 * 施工本身交给 {@link MaidBuildController} 每 tick 推进，不再经过 LLM。
 */
public class MaidBuildHandler implements MessageHandler {
    /** 一张图纸最多这么多格：够盖一座小屋，也不至于让一次请求吃掉太多主线程时间。 */
    private static final int MAX_CELLS = 2048;
    /** 相对锚点的最远偏移。 */
    private static final int MAX_OFFSET = 64;
    private static final int MAX_NAME = 40;

    private final MinecraftServer server;

    public MaidBuildHandler(MinecraftServer server) {
        this.server = server;
    }

    @Override
    public JsonObject handle(JsonObject request) {
        if (server == null) {
            return rejected("not_ready", MaidQueries.NOT_READY, "", 0);
        }
        JsonObject data = Envelope.data(request);
        EntityMaid maid = MaidQueries.findMaidById(server, Envelope.string(data, "maid_id"));
        if (maid == null) {
            return rejected("maid_not_found", MaidQueries.NOT_FOUND, "", 0);
        }
        if (MaidCondition.isUnconscious(maid)) {
            return rejected("maid_unconscious", "她昏迷不醒，叫不动", "", 0);
        }
        if (MaidQueries.carriedByPlayer(maid)) {
            return rejected("maid_carried", "玩家正抱着她，先放下来再让她盖", "", 0);
        }
        return build(server, maid, data);
    }

    private static JsonObject build(MinecraftServer server, EntityMaid maid, JsonObject data) {
        JsonObject anchorObject = objectOf(data, "anchor");
        if (anchorObject == null) {
            return rejected("bad_anchor", "没有给出施工起点", "", 0);
        }
        Integer anchorX = intOf(anchorObject, "x");
        Integer anchorY = intOf(anchorObject, "y");
        Integer anchorZ = intOf(anchorObject, "z");
        if (anchorX == null || anchorY == null || anchorZ == null) {
            return rejected("bad_anchor", "施工起点缺少坐标", "", 0);
        }
        BlockPos anchor = new BlockPos(anchorX, anchorY, anchorZ);
        Level level = maid.level();
        if (!level.isLoaded(anchor)) {
            return rejected("not_loaded", "施工起点还没加载出来", "", 0);
        }

        List<CellSpec> specs = new ArrayList<>();
        JsonElement cellsElement = data.get("cells");
        if (cellsElement == null || !cellsElement.isJsonArray()) {
            return rejected("no_cells", "图纸里没有格子", "", 0);
        }
        for (JsonElement element : cellsElement.getAsJsonArray()) {
            if (!element.isJsonObject()) {
                return rejected("bad_cell", "图纸里有一格写坏了", "", 0);
            }
            JsonObject cell = element.getAsJsonObject();
            Integer x = intOf(cell, "x");
            Integer y = intOf(cell, "y");
            Integer z = intOf(cell, "z");
            if (x == null || y == null || z == null) {
                return rejected("bad_cell", "图纸里有一格缺少坐标", "", 0);
            }
            if (Math.abs(x) > MAX_OFFSET || Math.abs(z) > MAX_OFFSET || y < -16 || y > 128) {
                return rejected("bad_cell", "图纸里有格子离锚点太远", "", 0);
            }
            Block block = blockOf(Envelope.string(cell, "block"));
            if (block == null) {
                return rejected(
                        "bad_cell", "图纸里有认不出的方块：" + Envelope.string(cell, "block"), "", 0);
            }
            Item item = block.asItem();
            if (item == net.minecraft.world.item.Items.AIR || !(item instanceof BlockItem)) {
                return rejected(
                        "bad_cell", "图纸里的方块没有能放下的物品：" + Envelope.string(cell, "block"), "", 0);
            }
            Float yaw = PlaceRules.yawOf(Envelope.string(cell, "facing"));
            specs.add(new CellSpec(x, y, z, block, yaw));
            if (specs.size() > MAX_CELLS) {
                return rejected("too_many_cells", "图纸超过了 " + MAX_CELLS + " 格", "", 0);
            }
        }
        if (specs.isEmpty()) {
            return rejected("no_cells", "图纸里没有格子", "", 0);
        }

        // 补料后接着盖：图纸没变、上一张还没盖完，就走回原来的工地接着放。图纸可能已经按
        // 她此刻的朝向转过了，签名按四个旋转方向归一；命中后直接用上次算好的世界坐标，
        // 免得半成品和新朝向对不上，也免得她跑远之后又另起一座。
        String signature = signatureOf(specs);
        MaidBuildController.Resume resume = MaidBuildController.resume(signature, maid);

        String name = Envelope.string(data, "name");
        if (name.isEmpty()) {
            name = "小木屋";
        }
        if (name.length() > MAX_NAME) {
            name = name.substring(0, MAX_NAME);
        }

        int baseY;
        List<MaidBuildController.Target> targets;
        if (resume != null) {
            anchor = resume.anchor();
            baseY = resume.baseY();
            targets = new ArrayList<>(resume.targets());
        } else {
            baseY = baseY(level, anchor, specs);
            targets = new ArrayList<>(specs.size());
            for (CellSpec spec : specs) {
                BlockPos pos = new BlockPos(anchor.getX() + spec.x(), baseY + spec.y(), anchor.getZ() + spec.z());
                targets.add(new MaidBuildController.Target(pos, spec.block(), spec.yaw()));
            }
        }
        for (MaidBuildController.Target target : targets) {
            BlockPos pos = target.pos();
            if (!level.isLoaded(pos)) {
                return rejected(
                        "not_loaded",
                        resume != null ? "上一座还没盖完的工地还没加载出来，先过去看看" : "图纸有一格还没加载出来",
                        name,
                        targets.size());
            }
            if (maid.hasRestriction() && !maid.isWithinRestriction(pos)) {
                return rejected("out_of_restriction", "图纸超出了她的活动范围", name, targets.size());
            }
        }

        ServerPlayer owner = MaidQueries.owner(maid);
        boolean creative = owner != null && owner.isCreative();
        // 创造模式下她不用背包里的材料；生存模式默认扣料，图纸也可以显式说不要扣
        boolean consume = !creative && !"false".equalsIgnoreCase(Envelope.string(data, "consume_materials"));

        MaidBuildController.start(server, maid, name, signature, anchor, baseY, targets, consume);

        JsonObject result = baseResult(name, targets.size());
        result.addProperty("accepted", true);
        result.addProperty("async", true);
        result.addProperty("resumed", resume != null);
        result.addProperty("creative", creative);
        result.addProperty("consume_materials", consume);
        return Envelope.response(Protocol.MAID_BUILD_RESULT, result);
    }

    /**
     * 图纸签名：同一份图纸的格子、方块与朝向，顺序无关，四个旋转方向算同一张图。
     *
     * <p>插件会按她当时的朝向旋转图纸，坐标与 facing 都跟着转；这里把四种转角各归一
     * 一次、取最小的那串，才能认出「这就是上一张图纸」。
     */
    private static String signatureOf(List<CellSpec> specs) {
        String best = "";
        for (int turn = 0; turn < 4; turn++) {
            List<String> parts = new ArrayList<>(specs.size());
            for (CellSpec spec : specs) {
                int x = spec.x();
                int z = spec.z();
                Float yaw = spec.yaw();
                for (int i = 0; i < turn; i++) {
                    int rotatedX = -z;
                    z = x;
                    x = rotatedX;
                    if (yaw != null) {
                        yaw = (yaw + 90.0F) % 360.0F;
                    }
                }
                String yawKey = yaw == null ? "" : Integer.toString(Math.round(yaw));
                parts.add(x + "," + spec.y() + "," + z + ","
                        + BuiltInRegistries.BLOCK.getKey(spec.block()) + "," + yawKey);
            }
            Collections.sort(parts);
            String candidate = String.join(";", parts);
            if (turn == 0 || candidate.compareTo(best) < 0) {
                best = candidate;
            }
        }
        return best;
    }

    /** 地板所在的最高地面之上那一层；没有地板格就按锚点原样。 */
    private static int baseY(Level level, BlockPos anchor, List<CellSpec> specs) {
        int highest = Integer.MIN_VALUE;
        for (CellSpec spec : specs) {
            if (spec.y() != 0) {
                continue;
            }
            int x = anchor.getX() + spec.x();
            int z = anchor.getZ() + spec.z();
            for (int y = anchor.getY() + 1; y >= anchor.getY() - 1; y--) {
                BlockPos pos = new BlockPos(x, y, z);
                if (!level.isLoaded(pos)) {
                    continue;
                }
                BlockState state = level.getBlockState(pos);
                if (state.isAir() || !state.getFluidState().isEmpty()) {
                    continue;
                }
                if (state.getCollisionShape(level, pos).isEmpty()) {
                    continue;
                }
                highest = Math.max(highest, y);
                break;
            }
        }
        return highest == Integer.MIN_VALUE ? anchor.getY() : highest + 1;
    }

    private static Block blockOf(String token) {
        String bare = token == null ? "" : token.trim();
        if (bare.isEmpty() || bare.startsWith("#")) {
            return null;
        }
        ResourceLocation id =
                ResourceLocation.tryParse(bare.contains(":") ? bare : "minecraft:" + bare);
        if (id == null) {
            return null;
        }
        Block block = BuiltInRegistries.BLOCK.get(id);
        return block == Blocks.AIR ? null : block;
    }

    private static JsonObject objectOf(JsonObject object, String key) {
        JsonElement element = object == null ? null : object.get(key);
        return element != null && element.isJsonObject() ? element.getAsJsonObject() : null;
    }

    private static Integer intOf(JsonObject object, String key) {
        JsonElement element = object == null ? null : object.get(key);
        if (element == null || !element.isJsonPrimitive() || !element.getAsJsonPrimitive().isNumber()) {
            return null;
        }
        return element.getAsInt();
    }

    private static JsonObject baseResult(String name, int cells) {
        JsonObject data = new JsonObject();
        data.addProperty("accepted", false);
        data.addProperty("async", false);
        data.addProperty("resumed", false);
        data.addProperty("name", name);
        data.addProperty("cells", cells);
        data.addProperty("creative", false);
        data.addProperty("consume_materials", true);
        data.addProperty("reason_code", "");
        data.addProperty("reason", "");
        return data;
    }

    private static JsonObject rejected(String reasonCode, String reason, String name, int cells) {
        JsonObject data = baseResult(name, cells);
        data.addProperty("reason_code", reasonCode);
        data.addProperty("reason", reason);
        return Envelope.response(Protocol.MAID_BUILD_RESULT, data);
    }

    /** 图纸里的一格：相对锚点的偏移、方块与朝向。 */
    private record CellSpec(int x, int y, int z, Block block, Float yaw) {
    }
}
