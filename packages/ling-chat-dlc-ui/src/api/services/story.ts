import { invoke } from "@tauri-apps/api/core";

// 清除剧本的持久化运行状态（周目记忆），下次进入从第一周目重新开始。
// 返回 true 表示确实有记忆被清掉。
export const resetScriptState = async (scriptName: string): Promise<boolean> => {
  try {
    return await invoke<boolean>("reset_script_state", { scriptName });
  } catch (error: any) {
    console.error("重置剧本记忆错误:", error);
    throw error;
  }
};

export interface ScriptGhostLock {
  locked: boolean;
  /** 剧情结束后的进入错误，重置该剧本记忆后解除。 */
  entry_error?: string;
  /** 锁定中时为该剧本 Assets 目录绝对路径（convertFileSrc 加载素材用） */
  asset_dir?: string;
}

// 删角色文件彩蛋（DDLC ghost menu 对应物）：.chr 被全删的剧本进入时锁成幽灵演出。
// 进入前实时查询，避免列表缓存过期——玩家可能刚在另一个窗口删完/放回文件。
export const checkScriptGhostLock = async (scriptName: string): Promise<ScriptGhostLock> => {
  try {
    return await invoke<ScriptGhostLock>("check_script_ghost_lock", {
      scriptName,
    });
  } catch (error: any) {
    console.error("检查剧本幽灵锁定错误:", error);
    // 判定失败宁可放行，不能把玩家正常剧本锁在门外
    return { locked: false };
  }
};
