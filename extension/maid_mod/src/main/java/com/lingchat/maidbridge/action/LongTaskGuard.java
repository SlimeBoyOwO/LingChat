package com.lingchat.maidbridge.action;

import com.github.tartaricacid.touhoulittlemaid.api.task.IMaidTask;
import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.github.tartaricacid.touhoulittlemaid.entity.task.TaskIdle;
import com.lingchat.maidbridge.action.core.MaidActionController;
import com.lingchat.maidbridge.action.attack.MaidAttackController;
import com.lingchat.maidbridge.action.mine.MaidMineController;

/**
 * 「她此刻手上有活」这一个判断。
 *
 * <p>给 {@code MaidFollowOwnerTaskMixin} 用：攻击、挖方块、走到床边，以及她自己界面里选的
 * 工作模式（农场、钓鱼……）都要她离开主人一段距离；车万女仆的跟随行为会在主人走远时直接
 * 把她传送回来，活就断了。有活期间取消那个行为的 start，活干完（或切回空闲）自动恢复——
 * 不碰她的活动半径，拾物、找床、找餐桌的范围保持车万女仆原样。
 */
public final class LongTaskGuard {
    private static final org.slf4j.Logger LOGGER =
            org.slf4j.LoggerFactory.getLogger("lingchat_maid_bridge");
    /** 上一次已经记过日志的任务女仆：每次任务只留一笔，排障用。 */
    private static String loggedMaid = "";

    private LongTaskGuard() {
    }

    public static boolean active(EntityMaid maid) {
        if (maid == null) {
            return false;
        }
        boolean running = MaidAttackController.isRunning(maid)
                || MaidMineController.isRunning(maid)
                || MaidActionController.isRunning(maid)
                // 她自己界面里那个工作模式也算「有任务」：农场、钓鱼这些干到一半被拽回来
                // 同样会中断；空闲模式才回到正常的跟随
                || hasWorkTask(maid);
        if (running) {
            String id = maid.getStringUUID();
            if (!id.equals(loggedMaid)) {
                loggedMaid = id;
                LOGGER.info("跟随行为暂时让路：{} 正在执行桥接的长任务", id);
            }
        } else {
            loggedMaid = "";
        }
        return running;
    }

    /** 她接着车万女仆自己的工作模式（农场、钓鱼、攻击……）而不是空闲。 */
    private static boolean hasWorkTask(EntityMaid maid) {
        IMaidTask task = maid.getTask();
        return task != null && !TaskIdle.UID.equals(task.getUid());
    }
}
