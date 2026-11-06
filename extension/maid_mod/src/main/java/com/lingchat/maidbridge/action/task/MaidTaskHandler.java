package com.lingchat.maidbridge.action.task;

import com.github.tartaricacid.touhoulittlemaid.api.task.FunctionCallSwitchResult;
import com.github.tartaricacid.touhoulittlemaid.api.task.IMaidTask;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.github.tartaricacid.touhoulittlemaid.entity.task.TaskManager;
import com.google.gson.JsonObject;
import com.lingchat.maidbridge.LingChatMaidBridge;
import com.lingchat.maidbridge.action.core.MaidActionController;
import com.lingchat.maidbridge.action.core.MaidActuator;
import com.lingchat.maidbridge.action.attack.MaidAttackController;
import com.lingchat.maidbridge.bridge.BridgeServer;
import com.lingchat.maidbridge.bridge.Envelope;
import com.lingchat.maidbridge.bridge.MessageHandler;
import com.lingchat.maidbridge.bridge.Protocol;
import com.lingchat.maidbridge.action.mine.MaidMineController;
import com.lingchat.maidbridge.action.place.MaidPlaceController;
import com.lingchat.maidbridge.maid.MaidCondition;
import com.lingchat.maidbridge.maid.MaidQueries;
import com.lingchat.maidbridge.perception.EventFrames;
import java.util.Optional;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.entity.schedule.Activity;

/**
 * {@code maid_set_task}：给她换一个工作模式（农场、钓鱼、攻击、空闲……）。
 *
 * <p>切的是车万女仆自己的任务，所以她的界面、存档、日程表跟玩家手动点出来的一模一样，
 * 我们不留副本。真正的「她现在会不会去干活」还看两件事，帧里如实带回去，措辞留给插件：
 * 一是她的日程表此刻在不在工作时间，二是她开着居家还是跟随。
 *
 * <p>切之前先把插件发起的动作收掉：那个控制器每 tick 写寻路目标，与工作模式的寻路会
 * 互相顶，两边都以为自己在指挥她。
 *
 * <p>切成功且确实换了的话顺带推一条 {@code maid_task_change} 事件：工具回执只在那一轮
 * 的上下文里，换工作这件事得在台词表里留一笔，否则过一会儿她就不知道自己现在在干什么。
 */
public class MaidTaskHandler implements MessageHandler {
    private final MinecraftServer server;

    public MaidTaskHandler(MinecraftServer server) {
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
        return switchTask(server, maid, Envelope.string(Envelope.data(request), "task"));
    }

    private static JsonObject switchTask(MinecraftServer server, EntityMaid maid, String rawTask) {
        ResourceLocation id = MaidTaskIds.resolve(rawTask);
        if (id == null) {
            return rejected("bad_task_id", "没有这个工作模式：" + rawTask, rawTask);
        }
        Optional<IMaidTask> found = TaskManager.findTask(id);
        if (found.isEmpty()) {
            return rejected("unknown_task", "没有这个工作模式：" + id, id.toString());
        }
        IMaidTask task = found.get();
        if (!task.isEnable(maid)) {
            // 界面里会把她用不了的模式置灰；这里同样不硬切，免得切过去她仍旧站着不动
            return rejected("task_disabled", "这个工作模式她现在用不了（条件没满足）：" + id, id.toString());
        }

        IMaidTask previous = maid.getTask();
        boolean changed = task != previous;
        // 换工作与追打、挖掘会互相抢寻路，先把还没做完的收掉
        MaidAttackController.cancel();
        MaidMineController.cancel(server);
        MaidPlaceController.cancel(server);
        String replaced = MaidActionController.cancelCurrent(server);
        boolean stoodUp = MaidActuator.standUp(maid);
        if (changed) {
            maid.setTask(task);
        }
        // 换过去的这个模式要用的工具（锄头、剪刀、钓竿）她自己会从背包里拿到手上
        FunctionCallSwitchResult switchResult = task.onFunctionCallSwitch(maid);

        JsonObject data = new JsonObject();
        data.addProperty("accepted", true);
        data.addProperty("task", id.toString());
        data.addProperty("previous_task", previous.getUid().toString());
        // 中文名在插件那张表里；它不认识的（旁人加的模式）才用这句英文摘要兜底
        data.addProperty("summary", task.getMaidActionSummary());
        data.addProperty("changed", changed);
        data.addProperty("switch_result", switchResult.name());
        data.addProperty("work_time", maid.getScheduleDetail() == Activity.WORK);
        data.addProperty("home_mode", maid.isHomeModeEnable());
        data.addProperty("stood_up", stoodUp);
        data.addProperty("replaced_action", replaced == null ? "" : replaced);
        data.addProperty("reason_code", "");
        data.addProperty("reason", "");

        if (changed) {
            broadcastChange(maid, id, task.getMaidActionSummary());
        }
        return Envelope.response(Protocol.MAID_SET_TASK_RESULT, data);
    }

    private static void broadcastChange(EntityMaid maid, ResourceLocation id, String summary) {
        BridgeServer bridge = LingChatMaidBridge.server();
        if (bridge == null || !bridge.hasClients()) {
            return;
        }
        JsonObject frame = EventFrames.base("maid_task_change", maid.getStringUUID());
        frame.addProperty("task", id.toString());
        frame.addProperty("summary", summary);
        bridge.broadcastEvent(frame);
    }

    private static JsonObject rejected(String reasonCode, String reason, String task) {
        JsonObject data = new JsonObject();
        data.addProperty("accepted", false);
        data.addProperty("task", task == null ? "" : task);
        data.addProperty("previous_task", "");
        data.addProperty("summary", "");
        data.addProperty("changed", false);
        data.addProperty("switch_result", "");
        data.addProperty("work_time", false);
        data.addProperty("home_mode", false);
        data.addProperty("stood_up", false);
        data.addProperty("replaced_action", "");
        data.addProperty("reason_code", reasonCode);
        data.addProperty("reason", reason);
        return Envelope.response(Protocol.MAID_SET_TASK_RESULT, data);
    }
}
