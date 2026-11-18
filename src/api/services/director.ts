import { invoke, type Channel } from "@tauri-apps/api/core";

/**
 * 导演 Agent 的后端接口（主聊天页右侧侧栏）。
 *
 * 只做 invoke 封装，不含业务逻辑。命令均为 `director_*`。
 * 注意：运行层尚未接入后端时，这些命令会直接因「命令不存在」而 reject，
 * 调用方（stores/modules/director）会退回「服务未就绪」空态，不弹错。
 */

// ============================================================
// DTO
// ============================================================

/** OpenAI 格式的工具调用（assistant 消息携带）。 */
export interface DirectorToolCall {
  id: string;
  type: string;
  function: { name: string; arguments: string };
}

/** 一条导演消息（user / assistant / tool）。 */
export interface DirectorMessage {
  /** 数据库 id；0 表示尚未落库（自由对话或本轮刚产生）。 */
  id: number;
  role: "user" | "assistant" | "tool" | "system";
  content: string;
  reasoning: string | null;
  toolCalls: DirectorToolCall[] | null;
  toolCallId: string | null;
  createdAt: string;
}

/** 侧栏打开 / 存档切换后拉取的整体状态。 */
export interface DirectorState {
  /** 后端是否具备运行条件（LLM 槽位已解析）。false 时前端只显示空态。 */
  available: boolean;
  /** 是否绑定存档（false = 自由对话，消息只在内存）。 */
  saveBound: boolean;
  saveId: number | null;
  running: boolean;
  messages: DirectorMessage[];
}

/** 装配后的上下文统计。 */
export interface DirectorContextStats {
  systemChars: number;
  historyChars: number;
  messageCount: number;
  /** 因历史预算被整块裁掉的消息数。 */
  droppedMessages: number;
  /** 是否发生过截断（含台词段与历史预算）。 */
  truncated: boolean;
}

/** system 的分段视图，供调试页分区展示。 */
export interface DirectorContextSection {
  /** persona / scene / roles / recent */
  key: string;
  title: string;
  content: string;
}

/** 一次请求的上下文快照（只存内存，供「查看记忆」展示）。 */
export interface DirectorContext {
  source: "last_run" | "rebuilt";
  builtAt: string;
  provider: string | null;
  model: string | null;
  systemPrompt: string;
  sections: DirectorContextSection[];
  /** 实际发给 LLM 的消息序列（含 system 与历史）。 */
  messages: {
    role: string;
    content: string | null;
    tool_calls?: DirectorToolCall[] | null;
    tool_call_id?: string | null;
  }[];
  stats: DirectorContextStats;
}

/** 周期小剧本的运行模式。 */
export interface DirectorMode {
  enabled: boolean;
  interval: number;
  /** 上次触发生成时的对话序号；null 表示尚未生成过。 */
  lastGeneratedSeq: number | null;
  pendingDraft: DirectorDraft | null;
}

/** 待确认的小剧本草案：一条条剧本事件，确认后按序应用。 */
export interface DirectorDraftEvent {
  event: string;
  params: Record<string, unknown>;
}

export interface DirectorDraft {
  status: "idle" | "generating" | "pending" | "applying" | "failed";
  events: DirectorDraftEvent[];
  error: string | null;
  createdSeq: number | null;
}

/** 确认应用草案的结果。 */
export interface DirectorConfirmResult {
  ok: boolean;
  applied: number;
  error: string | null;
}

/** 后端流式事件（serde tag="type"，snake_case）。 */
export type DirectorAgentEvent =
  | { type: "status"; content: string }
  | { type: "message_delta"; content: string }
  | { type: "reasoning"; content: string }
  | {
      type: "tool_call";
      call_id: string;
      tool: string;
      args: Record<string, unknown>;
    }
  | {
      type: "tool_result";
      call_id: string;
      tool: string;
      ok: boolean;
      output: string;
      error: string | null;
    }
  | { type: "done"; final_text: string; message_id: number }
  | { type: "error"; message: string };

// ============================================================
// 命令
// ============================================================

export const getDirectorState = () => invoke<DirectorState>("director_get_state");

/** 开始一轮对话。返回本次用户消息的 DB id（0 = 自由对话未落库）。 */
export const startDirectorChat = (message: string, channel: Channel<DirectorAgentEvent>) =>
  invoke<number>("director_start_chat", { message, channel });

/** 请求停止：后端在 chunk 之间中断并把已产出的正文落库。 */
export const stopDirectorChat = () => invoke<void>("director_stop_chat");

/** 清空当前存档的导演线（DB + 内存）。 */
export const clearDirector = () => invoke<void>("director_clear");

/** 上下文快照：默认返回上次实际请求的那份，rebuild=true 按当前世界状态重算。 */
export const getDirectorContext = (rebuild = false) =>
  invoke<DirectorContext>("director_get_context", { rebuild });

/** 工具调用历史（筛 assistant.tool_calls 与 tool 结果消息）。 */
export const getDirectorToolHistory = () => invoke<DirectorMessage[]>("director_get_tool_history");

// ============================================================
// 周期小剧本（导演模式）
// ============================================================

export const getDirectorMode = () => invoke<DirectorMode>("director_get_mode");

/** enabled 必传；intervalRounds 省略时后端沿用当前值。 */
export const setDirectorMode = (enabled: boolean, intervalRounds?: number) =>
  invoke<void>("director_set_mode", {
    enabled,
    ...(intervalRounds !== undefined ? { intervalRounds } : {}),
  });

export const getDirectorDraft = () => invoke<DirectorDraft | null>("director_get_draft");

export const confirmDirectorDraft = () =>
  invoke<DirectorConfirmResult>("director_confirm_draft");

export const discardDirectorDraft = () => invoke<void>("director_discard_draft");
