import { useGameStore } from "@/stores/modules/game";
import { setRoleAvatarStatic } from "@/api/services/character";

/**
 * Live2D 模型文件缺失时的自动回退：把角色切回静态立绘并落盘。
 *
 * 由舞台在收到 `modelMissing` 时调用（只在模型源确实取不到时发出，引擎故障不发）。
 * 落盘是必须的：不落盘的话每次进舞台还会再试一遍，每次都白建一套渲染上下文。
 *
 * 两个场景一起切——同一份模型服务主对话与桌宠，只切一个会让另一个仍然空着。
 */
export function useLive2dFallback() {
  const gameStore = useGameStore();
  /** 已经发起过落盘的角色，避免同一角色重复写文件 */
  const persisting = new Set<number>();

  return (roleId: number) => {
    const role = gameStore.gameRoles[roleId];
    if (role) {
      // 先改内存：舞台据此立刻退成静态，不必等落盘往返
      role.avatarMode = "image";
      role.avatarModeP = "image";
    }

    if (persisting.has(roleId)) return;
    persisting.add(roleId);
    void setRoleAvatarStatic(roleId).catch((error: unknown) => {
      persisting.delete(roleId);
      console.warn(`[Live2D] 回退静态立绘落盘失败: role ${roleId}`, error);
    });
  };
}
