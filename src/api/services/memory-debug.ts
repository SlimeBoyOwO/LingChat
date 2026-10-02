import { invoke } from "@tauri-apps/api/core";

/**
 * 永久记忆（MemoryBank）调试数据的读取入口。
 *
 * 对应后端 `src-tauri/src/api/memory.rs`：**只读**，不触发压缩、不写回记忆库、不改存档。
 * 后续若加写操作（手动触发压缩 / 清空记忆 / 重置指针），命令也放在那边，
 * 并复用同一套返回结构。
 *
 * 命名提醒：外层 DTO 由 Rust 侧 `#[serde(rename_all = "camelCase")]` 输出 camelCase，
 * 但**嵌套的 `GameMemoryBank` 与 `LlmMessage` 没有 rename_all**，键名保持 snake_case
 * （如 `last_processed_global_idx`、`image_data_url`）。下面的接口按实际键名写。
 */

/** 单个记忆段的「存储字符数 / 注入上限」对照。 */
export interface MemorySectionStat {
  key: "short_term" | "long_term" | "user_info" | "promises";
  /** 存储真源里的字符数（记忆库不截断）。 */
  storedChars: number;
  /** 该段的注入上限；0 = 不截断。 */
  limit: number;
  /** 注入时是否会被截断（limit != 0 && storedChars > limit）。 */
  truncated: boolean;
}

export interface GameMemoryBankMeta {
  /** 压缩指针：已归档到 `line_list` 的第几条（全局下标）。 */
  last_processed_global_idx: number;
  updated_at: string;
}

export interface GameMemoryBankData {
  short_term: string;
  long_term: string;
  user_info: string;
  promises: string;
}

export interface GameMemoryBank {
  schema_version: number;
  meta: GameMemoryBankMeta;
  data: GameMemoryBankData;
}

export interface FunctionCall {
  name: string;
  /** JSON 字符串，展示时通常需要二次 JSON.parse。 */
  arguments: string;
}

export interface ToolCall {
  id: string;
  type: string;
  function: FunctionCall;
}

/** 与 Rust `ai_service::types::LlmMessage` 对应；可选字段在 None 时整个不出现。 */
export interface LlmMessage {
  role: string;
  content: string;
  tool_calls?: ToolCall[];
  tool_call_id?: string;
  image_data_url?: string;
}

/** 永久记忆运行时的只读快照。 */
export interface MemorySystemSnapshot {
  /** 运行时真源（不是 GameRole 上那个滞后副本）。键名为 snake_case。 */
  bank: GameMemoryBank;
  sections: MemorySectionStat[];
  /** LLM 实际收到的 system 记忆块（已按上限截断；`short_term` 不在这里）。 */
  injectedSystemText: string;
  /** LLM 实际收到的短期回顾前缀；空串表示完全不注入。 */
  injectedShortTermText: string;
  enabled: boolean;
  /** 构造时烘焙的角色名；压缩提示词里用的是它，改名后不会更新。 */
  aiName: string;
  /** 以下 interval / window 是**运行时生效值**（构造时定死，改配置需重启）。 */
  updateInterval: number;
  recentWindow: number;
  pointerRaw: number;
  pointerEffective: number;
  /** 指针越界（撤回/读档/清空后历史变短）；下次触发时会被重置为 0。 */
  pointerOutOfRange: boolean;
  /** 指针之后、该角色可见的非 system 台词数 = 触发进度。 */
  accumulatedVisibleCount: number;
  /** 当前台词历史总行数（用于解释指针）。 */
  lineCount: number;
  isUpdating: boolean;
  /** 有未同步的后台结果；**不等于**"压缩刚完成"（指针修正与空区间推进也会置位）。 */
  hasPending: boolean;
  failCount: number;
  historyRevision: number;
  /** 失败冷却剩余毫秒；0 = 不在冷却中。 */
  cooldownRemainingMs: number;
}

/** 运行时缺失/不可用的原因。 */
export type MemoryRuntimeNote = "global_switch_off" | "no_llm_configured" | "not_synced_yet";

export interface MemoryDebugRole {
  roleId: number;
  displayName: string | null;
  /** 该角色的永久记忆运行时是否已创建。 */
  runtimePresent: boolean;
  /** `null` = 运行时存在且已启用。 */
  runtimeNote: MemoryRuntimeNote | null;
  /** 运行时开关（全局 `use_persistent_memory` 在创建时的取值）。 */
  enabled: boolean | null;
}

export interface MemoryDebugOverview {
  activeSaveId: number | null;
  /** `false` = 自由对话：记忆只在内存里，重启即失。 */
  activeSavePersistent: boolean;
  currentRoleId: number | null;
  /** 角色清单是否来自有序的 `onstage_role_ids`。 */
  rolesFromOnstage: boolean;
  roles: MemoryDebugRole[];
}

export interface RoleMemorySnapshot {
  roleId: number;
  displayName: string | null;
  runtime: MemorySystemSnapshot;
  /** 组装好、下一轮请求将以此为基础的上下文（不含刚发出的那条 user 消息）。 */
  context: LlmMessage[];
}

export const getMemoryDebugOverview = async (): Promise<MemoryDebugOverview> => {
  try {
    return (await invoke("get_memory_debug_overview")) as MemoryDebugOverview;
  } catch (error) {
    console.error("Error fetching memory debug overview:", error);
    throw error;
  }
};

export const getRoleMemorySnapshot = async (roleId: number): Promise<RoleMemorySnapshot> => {
  try {
    return (await invoke("get_role_memory_snapshot", {
      roleId,
    })) as RoleMemorySnapshot;
  } catch (error) {
    console.error(`Error fetching memory snapshot for role ${roleId}:`, error);
    throw error;
  }
};
