package com.lingchat.maidbridge.action;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.maid.MaidQueries;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;

/**
 * 动作前后这几秒别让她自己乱逛。
 *
 * <p>车万女仆的空闲、工作与休息日程里都挂着「四处看看 + 随机走动」，一次能把她带走好几格：
 * 刚走到主人身边又被溜达开、刚走到床头又走远了，玩家看到的就是「叫她过来她没过来」。动作在
 * 跑时一直按着，收尾后再按一会儿——等玩家这一轮的话说完（插件收到最后一句上屏就发
 * {@code maid_settle}），最迟十秒也会放开。主人走远到需要她跟随时立刻放开，免得该跟的时候
 * 还站在原地。
 *
 * <p>只管她自己的随机游荡：跟随、干活与寻路一概不动。有活时的跟随让路由
 * {@link LongTaskGuard} 负责，和这里互不牵扯。
 */
public final class WanderGuard {
    /** 动作收尾后再按多久（tick，20 = 1 秒）：等模型把这一轮的话说完。 */
    private static final int SETTLE_TICKS = 200;

    private static String maidId = "";
    private static boolean acting;
    private static int settleTicks;

    private WanderGuard() {
    }

    /** 动作受理：从现在起按住她的随机游荡。 */
    public static void begin(EntityMaid maid) {
        if (maid == null) {
            return;
        }
        maidId = maid.getStringUUID();
        acting = true;
        settleTicks = 0;
    }

    /** 动作收尾：开始倒数，等她说完话再放开。 */
    public static void end(String id) {
        if (!id.equals(maidId)) {
            return;
        }
        acting = false;
        settleTicks = SETTLE_TICKS;
    }

    /** 插件说这一轮已经说完话了：动作不在跑就可以放开了。 */
    public static void spoken(String id) {
        if (!id.equals(maidId) || acting) {
            return;
        }
        clear();
    }

    /** 每服务端 tick 推一次：倒数收尾，主人走远就当场放开。 */
    public static void tick(MinecraftServer server) {
        if (acting || settleTicks <= 0) {
            return;
        }
        if (ownerTooFar(server)) {
            clear();
            return;
        }
        if (--settleTicks <= 0) {
            clear();
        }
    }

    /** 这只女仆此刻的随机游荡要不要按住。 */
    public static boolean holds(EntityMaid maid) {
        return maid != null && maidId.equals(maid.getStringUUID()) && (acting || settleTicks > 0);
    }

    public static void clear() {
        maidId = "";
        acting = false;
        settleTicks = 0;
    }

    private static boolean ownerTooFar(MinecraftServer server) {
        EntityMaid maid = MaidQueries.findMaidById(server, maidId);
        if (maid == null) {
            // 区块卸载：先挂着，回来再说
            return false;
        }
        ServerPlayer owner = MaidQueries.owner(maid);
        if (owner == null || owner.level() != maid.level()) {
            return true;
        }
        return maid.distanceTo(owner) > Math.max(3.0F, maid.getRestrictRadius() - 2.0F);
    }
}
