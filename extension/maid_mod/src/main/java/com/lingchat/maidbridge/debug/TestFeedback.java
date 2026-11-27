package com.lingchat.maidbridge.debug;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.action.nav.NavFailure;
import com.lingchat.maidbridge.bridge.Envelope;
import java.util.Locale;
import java.util.UUID;
import net.minecraft.network.chat.Component;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;

/**
 * 调试指令的异步回执：指令只负责开工，结束原因事后发到执行者的聊天栏。
 *
 * <p>动作与施工各留一个等待槽，互不顶替：先开施工再喊「过来」时，施工收尾仍是施工
 * 自己的交代。按女仆的 UUID 认领，别的女仆收工不会误报；发之前就清掉，重复收尾只报一次。
 */
public final class TestFeedback {
    private static final String ACTION_TAG = "[走过来测试] ";
    private static final String BUILD_TAG = "[施工测试] ";

    private static final class Slot {
        UUID requester;
        String maidId = "";
        String label = "";
        /** 这一趟的寻路失败原因已经报过一次了。 */
        boolean noted;
    }

    private static final Slot ACTION = new Slot();
    private static final Slot BUILD = new Slot();

    private TestFeedback() {
    }

    /** 等一次「走过来」的收尾。 */
    public static void expectAction(ServerPlayer player, EntityMaid maid) {
        ACTION.requester = player.getUUID();
        ACTION.maidId = maid.getStringUUID();
        ACTION.noted = false;
    }

    /**
     * 走来走去的寻路判了走不通：先把原因报一次，收尾那句话随后照常发。
     *
     * <p>只在调试指令等着的时候报；报过一次就不再刷屏。
     */
    public static void noteNavFailure(EntityMaid maid, NavFailure failure, String phase,
            boolean hasScaffold, String detail) {
        if (maid == null || ACTION.requester == null || ACTION.noted
                || !ACTION.maidId.equals(maid.getStringUUID())) {
            return;
        }
        ACTION.noted = true;
        String text = "[走过来测试] 寻路判了走不通："
                + (failure == null ? "原因不明" : navText(failure) + "（" + failure.name() + "）")
                + "｜阶段：" + phase
                + (hasScaffold ? "" : "｜她身上没有能垫的方块")
                + (detail == null || detail.isEmpty() ? "" : "｜" + detail);
        say(ACTION, maid.level().getServer(), text);
    }

    /** 等一次施工的收尾。 */
    public static void expectBuild(ServerPlayer player, EntityMaid maid, String name) {
        BUILD.requester = player.getUUID();
        BUILD.maidId = maid.getStringUUID();
        BUILD.label = name;
    }

    /** 走过来结束了：到位、走不到、被打断都报。 */
    public static void actionDone(
            EntityMaid maid, String status, String code, String reason, int elapsedTicks) {
        if (maid == null || !claim(ACTION, maid.getStringUUID())) {
            return;
        }
        String body;
        if ("done".equals(status)) {
            body = "到位了";
        } else if ("cancelled".equals(status)) {
            body = "被打断了" + detail(reason, code);
        } else {
            body = "没走到" + detail(reason, code);
        }
        finish(ACTION, maid.level().getServer(),
                ACTION_TAG + body + "（用时 " + seconds(elapsedTicks) + "）");
    }

    /** 施工收工：盖完、盖了一部分、没盖成都报，带上失败原因。 */
    public static void buildDone(
            MinecraftServer server, String maidId, String status, String code, String text,
            int placed, int total, int ticks) {
        if (!claim(BUILD, maidId)) {
            return;
        }
        finish(BUILD, server,
                BUILD_TAG + buildText(BUILD.label, status, code, text, placed, total, ticks));
    }

    /** 图纸已经盖好、或者一块都放不了时施工器当场收工，没有异步回执可等。 */
    public static void buildNow(ServerPlayer player, JsonObject status) {
        if (player == null) {
            return;
        }
        String text = buildText(
                Envelope.string(status, "name"),
                Envelope.string(status, "status"),
                Envelope.string(status, "reason_code"),
                Envelope.string(status, "reason"),
                intOf(status, "placed"),
                intOf(status, "total"),
                0);
        player.sendSystemMessage(Component.literal(BUILD_TAG + text));
    }

    /** 停服时清掉等着的两件。 */
    public static void reset() {
        clear(ACTION);
        clear(BUILD);
    }

    /** 这一笔是不是正在等的那一件；是就当场认领，之后不会再报第二次。 */
    private static boolean claim(Slot slot, String maidId) {
        if (slot.requester == null || !slot.maidId.equals(maidId)) {
            return false;
        }
        return true;
    }

    private static String buildText(
            String name, String status, String code, String text,
            int placed, int total, int ticks) {
        String body;
        if ("done".equals(status)) {
            body = name + "盖完了，" + placed + "/" + total + " 块";
        } else if ("partial".equals(status)) {
            body = name + "盖了一部分，" + placed + "/" + total + " 块，还差 "
                    + Math.max(0, total - placed) + " 块；原因：" + detail(text, code);
        } else {
            body = name + "没盖成，" + placed + "/" + total + " 块；原因：" + detail(text, code);
        }
        return body + "（用时 " + seconds(ticks) + "）";
    }

    /** 原因先写中文说明，代码跟在括号里方便对日志；两样都没有就给个空串。 */
    private static String detail(String text, String code) {
        String reason = text == null ? "" : text;
        String id = code == null ? "" : code;
        if (reason.isEmpty() && id.isEmpty()) {
            return "";
        }
        if (reason.isEmpty()) {
            return "（" + id + "）";
        }
        return id.isEmpty() ? "：" + reason : "：" + reason + "（" + id + "）";
    }

    private static String seconds(int ticks) {
        return String.format(Locale.ROOT, "%.1f", ticks / 20.0) + " 秒";
    }

    /** 寻路失败原因的日常说法。搜空的两种写成「图搜空了」，免得读成周围没有落脚点。 */
    private static String navText(NavFailure failure) {
        return switch (failure) {
            case NO_STAND -> "落脚点图搜空了，没找到到得了目标附近的位置";
            case NO_PATH -> "有地方站，但走不过去";
            case NO_HEADROOM -> "头顶太低，跳不起来也垫不上去";
            case OUT_OF_REACH -> "站到了也够不着目标";
            case TIMEOUT -> "太久没有靠近过目标";
            case SEARCH_LIMIT -> "搜到节点上限，这一趟没算完";
        };
    }

    private static void finish(Slot slot, MinecraftServer server, String text) {
        say(slot, server, text);
        clear(slot);
    }

    private static void say(Slot slot, MinecraftServer server, String text) {
        ServerPlayer player = slot.requester == null || server == null
                ? null
                : server.getPlayerList().getPlayer(slot.requester);
        if (player != null) {
            player.sendSystemMessage(Component.literal(text));
        }
    }

    private static void clear(Slot slot) {
        slot.requester = null;
        slot.maidId = "";
        slot.label = "";
        slot.noted = false;
    }

    private static int intOf(JsonObject object, String key) {
        if (object == null) {
            return 0;
        }
        JsonElement element = object.get(key);
        return element != null && element.isJsonPrimitive() && element.getAsJsonPrimitive().isNumber()
                ? element.getAsInt()
                : 0;
    }
}
