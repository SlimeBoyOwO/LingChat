import { invoke, type Channel } from "@tauri-apps/api/core";

export interface DirectorToolCall {
  id: string;
  type: string;
  function: { name: string; arguments: string };
}

export interface DirectorMessage {
  id: number;
  role: "user" | "assistant" | "tool" | "system";
  content: string;
  reasoning: string | null;
  toolCalls: DirectorToolCall[] | null;
  toolCallId: string | null;
  createdAt: string;
}

export interface DirectorState {
  available: boolean;
  saveBound: boolean;
  saveId: number | null;
  running: boolean;
  messages: DirectorMessage[];
}

export interface DirectorContextStats {
  systemChars: number;
  historyChars: number;
  messageCount: number;
  droppedMessages: number;
  truncated: boolean;
}

export interface DirectorContextSection {
  key: string;
  title: string;
  content: string;
}

export interface DirectorContext {
  source: "last_run" | "rebuilt";
  builtAt: string;
  provider: string | null;
  model: string | null;
  systemPrompt: string;
  sections: DirectorContextSection[];
  messages: {
    role: string;
    content: string | null;
    tool_calls?: DirectorToolCall[] | null;
    tool_call_id?: string | null;
  }[];
  stats: DirectorContextStats;
}

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

export interface DirectorMode {
  enabled: boolean;
  interval: number;
  lastGeneratedSeq: number | null;
  pendingDraft: DirectorDraft | null;
  status: DirectorDraft["status"];
}

export interface DirectorConfirmResult {
  ok: boolean;
  applied: number;
  error: string | null;
}

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

export const getDirectorState = () => invoke<DirectorState>("director_get_state");

export const startDirectorChat = (message: string, channel: Channel<DirectorAgentEvent>) =>
  invoke<number>("director_start_chat", { message, channel });

export const stopDirectorChat = () => invoke<void>("director_stop_chat");
export const clearDirector = () => invoke<void>("director_clear");
export const getDirectorContext = (rebuild = false) =>
  invoke<DirectorContext>("director_get_context", { rebuild });
export const getDirectorToolHistory = () => invoke<DirectorMessage[]>("director_get_tool_history");
export const getDirectorMode = () => invoke<DirectorMode>("director_get_mode");

export const setDirectorMode = (enabled: boolean, intervalRounds?: number) =>
  invoke<DirectorMode>("director_set_mode", {
    enabled,
    ...(intervalRounds !== undefined ? { intervalRounds } : {}),
  });

export const getDirectorDraft = () => invoke<DirectorDraft | null>("director_get_draft");
export const confirmDirectorDraft = () => invoke<DirectorConfirmResult>("director_confirm_draft");
export const discardDirectorDraft = () => invoke<DirectorDraft | null>("director_discard_draft");
