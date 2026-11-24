package com.lingchat.maidbridge.action.construction;

import com.github.tartaricacid.touhoulittlemaid.entity.passive.EntityMaid;
import com.lingchat.maidbridge.action.construction.MaidBuildController.Run;
import com.lingchat.maidbridge.action.nav.Navigator;
import net.minecraft.server.MinecraftServer;

/**
 * 施工的四个相位：扫描、赶路、放置与间隔、收尾。
 *
 * <p>相位不单独存，每个 tick 由运行状态推出来：有寻路在跑就是赶路，冷却没走完就是间隔，
 * 收尾开始后就是 DONE，其余都是扫描。切换时跑进入与退出钩子；走路、跑跳这些子状态
 * 都属于寻路自己的内部状态，不再摊到这一层。
 */
public enum BuildPhase {
    /** 取一格判定：就地放、叫挖掘，还是叫寻路。 */
    SCAN,
    /** 寻路正把她送到能动手的位置。 */
    PATHING,
    /** 刚放下或刚收完一遍，歇几个 tick。 */
    PLACE_COOLDOWN,
    /** 收尾：走去实地、拆净垫脚，然后推收尾事件。 */
    DONE;

    static BuildPhase of(Run run) {
        if (run.finishing) {
            return DONE;
        }
        if (run.nav != null) {
            return PATHING;
        }
        if (run.cooldown > 0) {
            return PLACE_COOLDOWN;
        }
        return SCAN;
    }

    /** 相位切换：离开旧相位、进入新相位各跑一次钩子。 */
    static void to(MinecraftServer server, Run run, EntityMaid maid, BuildPhase next) {
        if (run.phase == next) {
            return;
        }
        run.phase.exit(run, maid);
        run.phase = next;
        next.enter(run, maid);
    }

    void enter(Run run, EntityMaid maid) {
        switch (this) {
            case DONE -> run.standFor = null;
            case PATHING -> run.stand = null;
            default -> {
            }
        }
    }

    void exit(Run run, EntityMaid maid) {
        switch (this) {
            case PATHING -> {
                if (run.nav != null && run.nav.status() == Navigator.Status.RUNNING) {
                    run.nav.cleanScaffold(maid);
                }
            }
            case DONE -> MaidBuildController.stopNav(run, maid);
            default -> {
            }
        }
    }

    void tick(MinecraftServer server, Run run, EntityMaid maid) {
        switch (this) {
            case SCAN -> MaidBuildController.scan(server, run, maid);
            case PATHING -> MaidBuildController.pathing(server, run, maid);
            case PLACE_COOLDOWN -> MaidBuildController.cooldown(run);
            case DONE -> MaidBuildController.settling(server, run, maid);
        }
    }
}
