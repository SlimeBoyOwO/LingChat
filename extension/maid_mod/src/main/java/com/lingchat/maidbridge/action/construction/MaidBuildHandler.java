package com.lingchat.maidbridge.action.construction;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.action.mine.MineRules;
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
import net.minecraft.world.phys.BlockHitResult;

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
        // 准星指定：图纸以玩家指着的那一格为中心，它上面一格起算施工区
        boolean looked = anchorObject != null && boolOf(anchorObject, "looked");
        BlockPos anchor;
        if (looked) {
            ServerPlayer owner = MaidQueries.owner(maid);
            if (owner == null || owner.level() != maid.level()) {
                return rejected("player_away", "玩家不在这里，不知道要盖在哪", "", 0);
            }
            BlockHitResult gaze = MaidQueries.gazeHit(owner, MineRules.ANCHOR_RAY);
            if (gaze == null) {
                return rejected("no_target", "先看向要盖的那片地面，再让她开工", "", 0);
            }
            anchor = gaze.getBlockPos();
        } else if (anchorObject != null) {
            Integer anchorX = intOf(anchorObject, "x");
            Integer anchorY = intOf(anchorObject, "y");
            Integer anchorZ = intOf(anchorObject, "z");
            if (anchorX == null || anchorY == null || anchorZ == null) {
                return rejected("bad_anchor", "施工起点缺少坐标", "", 0);
            }
            anchor = new BlockPos(anchorX, anchorY, anchorZ);
        } else {
            return rejected("bad_anchor", "没有给出施工起点", "", 0);
        }
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
        // 准星指着别处再喊一次：那是另起一座，不是走回旧工地补料
        if (resume != null && looked && !nearSite(resume.targets(), anchor)) {
            resume = null;
        }

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

        logAccepted(level, name, anchor, baseY, looked, resume != null, targets);
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

    /** 受理时打一行现场：锚点、起点、最低层各格上下是什么方块，卡住时好对账。 */
    private static void logAccepted(
            Level level, String name, BlockPos anchor, int baseY, boolean looked, boolean resumed,
            List<MaidBuildController.Target> targets) {
        int lowest = Integer.MAX_VALUE;
        for (MaidBuildController.Target target : targets) {
            lowest = Math.min(lowest, target.pos().getY());
        }
        StringBuilder sample = new StringBuilder();
        int shown = 0;
        for (MaidBuildController.Target target : targets) {
            BlockPos pos = target.pos();
            if (pos.getY() != lowest || shown >= 8) {
                continue;
            }
            if (shown > 0) {
                sample.append("；");
            }
            sample.append(pos.getX()).append(',').append(pos.getY()).append(',').append(pos.getZ())
                    .append(" 本格=").append(blockName(level, pos))
                    .append(" 下方=").append(blockName(level, pos.below()));
            shown++;
        }
        MaidBuildController.LOGGER.info(
                "施工受理：{}，锚点 {}（{}），起点 y={}，最低层 y={}，共 {} 格，续建 {}，现场 {}",
                name, anchor, looked ? "准星指定" : "坐标指定", baseY, lowest,
                targets.size(), resumed, sample);
    }

    private static String blockName(Level level, BlockPos pos) {
        return level.isLoaded(pos)
                ? BuiltInRegistries.BLOCK.getKey(level.getBlockState(pos).getBlock()).toString()
                : "未加载";
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

    /** 找地面时向下看这么多格：山坡、台地底下差着好几层也要找得到。 */
    private static final int GROUND_SCAN = 16;

    /**
     * 图纸最低一层该落在哪。
     *
     * <p>先按锚点算：最低那一层的格子底下有依托就照旧。没有依托说明整张图纸悬空——
     * 插件给的锚点常常是她脚这一格，图纸最低层若是相对 y=1，直接按锚点算就会整座
     * 悬空一格，施工器连第一块都放不了（每一格都找不到能贴的面）。这时把最低层放到
     * 它脚下最近的地面上；正常图纸仍按锚点原样。
     */
    private static int baseY(Level level, BlockPos anchor, List<CellSpec> specs) {
        int lowest = Integer.MAX_VALUE;
        for (CellSpec spec : specs) {
            lowest = Math.min(lowest, spec.y());
        }
        int base = anchor.getY();
        if (lowest != 0 && grounded(level, anchor, specs, lowest, base)) {
            return base;
        }
        int ground = groundUnder(level, anchor, specs, lowest, base);
        return ground == Integer.MIN_VALUE ? base : ground + 1 - lowest;
    }

    /** 最低那一层至少有一格底下是实心的：图纸已经坐在地上，别动它。 */
    private static boolean grounded(
            Level level, BlockPos anchor, List<CellSpec> specs, int lowest, int base) {
        for (CellSpec spec : specs) {
            if (spec.y() != lowest) {
                continue;
            }
            BlockPos below = new BlockPos(
                    anchor.getX() + spec.x(), base + spec.y() - 1, anchor.getZ() + spec.z());
            if (!level.isLoaded(below)) {
                return true;
            }
            BlockState state = level.getBlockState(below);
            if (state.isAir() || !state.getFluidState().isEmpty()) {
                continue;
            }
            if (!state.getCollisionShape(level, below).isEmpty()) {
                return true;
            }
        }
        return false;
    }

    /** 最低那一层脚下的最高实心格；一层都找不到给 MIN_VALUE。 */
    private static int groundUnder(
            Level level, BlockPos anchor, List<CellSpec> specs, int lowest, int base) {
        int highest = Integer.MIN_VALUE;
        for (CellSpec spec : specs) {
            if (spec.y() != lowest) {
                continue;
            }
            int x = anchor.getX() + spec.x();
            int z = anchor.getZ() + spec.z();
            for (int y = base + lowest + 1; y >= base + lowest - GROUND_SCAN; y--) {
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
        return highest;
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

    private static boolean boolOf(JsonObject object, String key) {
        JsonElement element = object == null ? null : object.get(key);
        return element != null
                && element.isJsonPrimitive()
                && element.getAsJsonPrimitive().isBoolean()
                && element.getAsBoolean();
    }

    /**
     * 准星指着的地方是不是上一座没盖完的工地。
     *
     * <p>指回工地就接着盖；指着另一片地方就是要另起一座。范围按上一座的格子外扩一圈，
     * 指到墙上、屋顶上、地基边都算同一处工地。
     */
    private static boolean nearSite(List<MaidBuildController.Target> targets, BlockPos point) {
        int minX = Integer.MAX_VALUE;
        int maxX = Integer.MIN_VALUE;
        int minY = Integer.MAX_VALUE;
        int maxY = Integer.MIN_VALUE;
        int minZ = Integer.MAX_VALUE;
        int maxZ = Integer.MIN_VALUE;
        for (MaidBuildController.Target target : targets) {
            BlockPos pos = target.pos();
            minX = Math.min(minX, pos.getX());
            maxX = Math.max(maxX, pos.getX());
            minY = Math.min(minY, pos.getY());
            maxY = Math.max(maxY, pos.getY());
            minZ = Math.min(minZ, pos.getZ());
            maxZ = Math.max(maxZ, pos.getZ());
        }
        return point.getX() >= minX - 1 && point.getX() <= maxX + 1
                && point.getY() >= minY - 2 && point.getY() <= maxY + 2
                && point.getZ() >= minZ - 1 && point.getZ() <= maxZ + 1;
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
