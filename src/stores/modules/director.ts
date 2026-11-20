import { Channel, invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import * as api from "@/api/services/director";
import type {
  DirectorAgentEvent,
  DirectorConfirmResult,
  DirectorContext,
  DirectorDraft,
  DirectorMessage,
  DirectorMode,
  DirectorToolCall,
} from "@/api/services/director";

export type DirectorTab = "chat" | "context" | "tools" | "draft";
export const DIRECTOR_DEFAULT_WIDTH = 420;
export const DIRECTOR_MIN_WIDTH = 320;
export const DIRECTOR_MAX_WIDTH = 720;

export interface DirectorStreaming {
  content: string;
  reasoning: string;
}

export interface DirectorActiveTool {
  tool: string;
  args: Record<string, unknown>;
  status: "pending" | "running" | "ok" | "error";
  output: string | null;
  error: string | null;
}

export interface DirectorModeState {
  enabled: boolean;
  interval: number;
  lastGeneratedSeq: number | null;
}

export interface DirectorStoreState {
  open: boolean;
  width: number;
  activeTab: DirectorTab;
  ready: boolean;
  available: boolean;
  saveBound: boolean;
  saveId: number | null;
  saveTitle: string | null;
  running: boolean;
  stopping: boolean;
  clearing: boolean;
  messages: DirectorMessage[];
  streaming: DirectorStreaming | null;
  activeTools: Record<string, DirectorActiveTool>;
  status: string;
  error: string;
  context: DirectorContext | null;
  loadingContext: boolean;
  toolHistory: DirectorMessage[];
  loadingTools: boolean;
  directorMode: DirectorModeState;
  draft: DirectorDraft | null;
  loadingMode: boolean;
  savingMode: boolean;
  confirming: boolean;
  discarding: boolean;
  modeError: string;
  applyResult: DirectorConfirmResult | null;
}

export interface DirectorToolPair {
  call: DirectorToolCall;
  result?: DirectorMessage;
}

let runSeq = 0;
let sessionEpoch = 0;
let stateRequest = 0;
let contextRequest = 0;
let toolsRequest = 0;
let modeRequest = 0;
let modeForegroundRequest = 0;
let initPromise: Promise<unknown> | null = null;

const emptyMessage = (
  role: DirectorMessage["role"],
  content: string,
  extra: Partial<DirectorMessage> = {},
): DirectorMessage => ({
  id: 0,
  role,
  content,
  reasoning: null,
  toolCalls: null,
  toolCallId: null,
  createdAt: "",
  ...extra,
});

export function isDirectorToolResultOk(content: string): boolean {
  return directorToolFailure(content) === null;
}

export function summarizeDirectorArgs(args: string | Record<string, unknown>, limit = 60): string {
  let text: string;
  if (typeof args === "string") {
    text = args;
  } else {
    try {
      text = JSON.stringify(args);
    } catch {
      text = String(args);
    }
  }
  if (!text || text === "{}") return "";
  return text.length > limit ? text.slice(0, limit) + "…" : text;
}

/** 工具结果只从所属 assistant 后的连续 tool 消息中配对。 */
export function pairDirectorToolHistory(messages: DirectorMessage[]): DirectorToolPair[] {
  const pairs: DirectorToolPair[] = [];
  for (let index = 0; index < messages.length; index++) {
    const message = messages[index];
    if (message.role !== "assistant" || !message.toolCalls) continue;
    const results = new Map<string, DirectorMessage>();
    for (let cursor = index + 1; cursor < messages.length; cursor++) {
      const candidate = messages[cursor];
      if (candidate.role === "assistant" || candidate.role === "user") break;
      if (candidate.role === "tool" && candidate.toolCallId && !results.has(candidate.toolCallId)) {
        results.set(candidate.toolCallId, candidate);
      }
    }
    for (const call of message.toolCalls) {
      pairs.push({ call, result: results.get(call.id) });
    }
  }
  return pairs;
}

export function directorToolFailure(content: string): string | null {
  try {
    const parsed = JSON.parse(content) as {
      ok?: boolean;
      error?: { message?: unknown };
    };
    if (parsed.ok !== false) return null;
    return typeof parsed.error?.message === "string" ? parsed.error.message : content;
  } catch {
    return null;
  }
}

function applyMode(state: Pick<DirectorStoreState, "directorMode" | "draft">, mode: DirectorMode) {
  state.directorMode = {
    enabled: mode.enabled,
    interval: mode.interval,
    lastGeneratedSeq: mode.lastGeneratedSeq,
  };
  state.draft = mode.pendingDraft;
}

export const useDirectorStore = defineStore("director", {
  state: (): DirectorStoreState => ({
    open: false,
    width: DIRECTOR_DEFAULT_WIDTH,
    activeTab: "chat",
    ready: false,
    available: true,
    saveBound: false,
    saveId: null,
    saveTitle: null,
    running: false,
    stopping: false,
    clearing: false,
    messages: [],
    streaming: null,
    activeTools: {},
    status: "",
    error: "",
    context: null,
    loadingContext: false,
    toolHistory: [],
    loadingTools: false,
    directorMode: { enabled: false, interval: 5, lastGeneratedSeq: null },
    draft: null,
    loadingMode: false,
    savingMode: false,
    confirming: false,
    discarding: false,
    modeError: "",
    applyResult: null,
  }),

  persist: {
    key: "lingchat-director",
    exclude: [
      "ready",
      "available",
      "saveBound",
      "saveId",
      "saveTitle",
      "running",
      "stopping",
      "clearing",
      "messages",
      "streaming",
      "activeTools",
      "status",
      "error",
      "context",
      "loadingContext",
      "toolHistory",
      "loadingTools",
      "directorMode",
      "draft",
      "loadingMode",
      "savingMode",
      "confirming",
      "discarding",
      "modeError",
      "applyResult",
    ],
  },

  getters: {
    statusKind: (state): "unavailable" | "running" | "idle" => {
      if (!state.ready || !state.available) return "unavailable";
      return state.running ? "running" : "idle";
    },
  },

  actions: {
    async init() {
      if (this.ready) return;
      if (!initPromise) {
        initPromise = this.loadState().finally(() => {
          initPromise = null;
        });
      }
      await initPromise;
    },

    async loadState(): Promise<boolean> {
      const request = ++stateRequest;
      const epoch = sessionEpoch;
      const seq = runSeq;
      try {
        const state = await api.getDirectorState();
        if (request !== stateRequest || epoch !== sessionEpoch || seq !== runSeq) return false;
        this.ready = true;
        this.available = state.available;
        this.saveBound = state.saveBound;
        this.saveId = state.saveId;
        this.running = state.running;
        if (!this.streaming) this.messages = state.messages;
        this.error = "";
        await this.loadSaveTitle();
        return true;
      } catch (err) {
        if (request !== stateRequest || epoch !== sessionEpoch || seq !== runSeq) return false;
        if (!this.ready) this.available = false;
        this.error = String(err);
        return false;
      }
    },

    /** 存档变化立即作废旧会话请求，并停止旧轮次后再读取新会话。 */
    async switchSession(saveId: number | null) {
      if (this.ready && this.saveId === saveId) return;
      this.resetSession(saveId);
      const epoch = sessionEpoch;
      let stopError = "";
      try {
        await api.stopDirectorChat();
      } catch (err) {
        stopError = String(err);
      }
      if (epoch !== sessionEpoch) return;
      await this.loadState();
      if (epoch === sessionEpoch && stopError) this.error = stopError;
    },

    resetSession(saveId: number | null) {
      sessionEpoch++;
      runSeq++;
      stateRequest++;
      contextRequest++;
      toolsRequest++;
      modeRequest++;
      modeForegroundRequest++;
      this.ready = false;
      this.saveBound = saveId !== null;
      this.saveId = saveId;
      this.saveTitle = null;
      this.running = false;
      this.stopping = false;
      this.clearing = false;
      this.messages = [];
      this.streaming = null;
      this.activeTools = {};
      this.status = "";
      this.error = "";
      this.context = null;
      this.loadingContext = false;
      this.toolHistory = [];
      this.loadingTools = false;
      this.draft = null;
      this.loadingMode = false;
      this.savingMode = false;
      this.confirming = false;
      this.discarding = false;
      this.modeError = "";
      this.applyResult = null;
    },

    async loadSaveTitle() {
      const saveId = this.saveId;
      const epoch = sessionEpoch;
      this.saveTitle = null;
      if (saveId == null) return;
      try {
        const res = await invoke<{ saves: { id: number; title: string }[] }>("list_saves", {
          page: 1,
          pageSize: 50,
        });
        if (epoch === sessionEpoch && this.saveId === saveId) {
          this.saveTitle = res.saves.find((save) => save.id === saveId)?.title ?? null;
        }
      } catch (err) {
        console.warn("[Director] 读取存档名失败:", err);
      }
    },

    async send(text: string): Promise<boolean> {
      const content = text.trim();
      if (
        !content ||
        !this.ready ||
        !this.available ||
        this.running ||
        this.stopping ||
        this.clearing
      )
        return false;
      this.error = "";
      this.status = "";
      this.running = true;
      this.activeTools = {};
      const userMessage = emptyMessage("user", content);
      this.messages.push(userMessage);
      this.streaming = { content: "", reasoning: "" };
      const seq = ++runSeq;
      const epoch = sessionEpoch;
      const channel = new Channel<DirectorAgentEvent>();
      channel.onmessage = (event) => {
        if (seq === runSeq && epoch === sessionEpoch) this.handleEvent(event);
      };
      try {
        const id = await api.startDirectorChat(content, channel);
        if (seq === runSeq && epoch === sessionEpoch && id > 0) userMessage.id = id;
        return true;
      } catch (err) {
        if (seq !== runSeq || epoch !== sessionEpoch) return true;
        runSeq++;
        this.running = false;
        this.streaming = null;
        this.activeTools = {};
        this.status = "";
        this.error = String(err);
        const index = this.messages.indexOf(userMessage);
        if (index >= 0) this.messages.splice(index, 1);
        return false;
      }
    },

    handleEvent(event: DirectorAgentEvent) {
      const streaming = this.streaming;
      switch (event.type) {
        case "status":
          this.status = event.content;
          break;
        case "message_delta":
          if (streaming) streaming.content += event.content;
          break;
        case "reasoning":
          if (streaming) streaming.reasoning += event.content;
          break;
        case "tool_call":
          this.activeTools[event.call_id] = {
            tool: event.tool,
            args: event.args,
            status: "running",
            output: null,
            error: null,
          };
          break;
        case "tool_result": {
          const entry = this.activeTools[event.call_id];
          if (!entry) break;
          entry.status = event.ok ? "ok" : "error";
          entry.output = event.output;
          entry.error = event.error;
          break;
        }
        case "done":
          void this.settle(event.final_text, event.message_id);
          break;
        case "error":
          void this.settle("", 0, event.message);
          break;
      }
    },

    /** 终态先保留本地降级视图，再用后端完整消息序列替换。 */
    async settle(finalText: string, messageId: number, failure?: string) {
      const streaming = this.streaming;
      const epoch = sessionEpoch;
      const seq = ++runSeq;
      this.running = false;
      this.stopping = false;
      this.status = "";
      this.streaming = null;
      const content = streaming?.content || finalText;
      const reasoning = streaming?.reasoning || null;
      if (content || reasoning) {
        this.messages.push(
          emptyMessage("assistant", content, {
            id: messageId > 0 ? messageId : 0,
            reasoning,
          }),
        );
      }
      this.error = failure ?? "";
      this.activeTools = {};
      await this.loadState();
      if (epoch !== sessionEpoch || seq !== runSeq) return;
      if (failure) this.error = failure;
    },

    async stop() {
      if (!this.running || this.stopping) return;
      const seq = runSeq;
      const epoch = sessionEpoch;
      this.stopping = true;
      try {
        await api.stopDirectorChat();
      } catch (err) {
        if (epoch !== sessionEpoch || seq !== runSeq) return;
        this.stopping = false;
        this.error = String(err);
        return;
      }
      if (epoch !== sessionEpoch || seq !== runSeq) return;
      await this.settle("", 0);
    },

    async clear() {
      if (this.running || this.stopping || this.clearing) return;
      const epoch = sessionEpoch;
      this.clearing = true;
      try {
        await api.clearDirector();
        if (epoch !== sessionEpoch) return;
        this.messages = [];
        this.activeTools = {};
        this.context = null;
        this.toolHistory = [];
        this.error = "";
      } catch (err) {
        if (epoch === sessionEpoch) this.error = String(err);
      } finally {
        if (epoch === sessionEpoch) this.clearing = false;
      }
    },

    async loadContext(rebuild = false) {
      const request = ++contextRequest;
      const epoch = sessionEpoch;
      this.loadingContext = true;
      try {
        const context = await api.getDirectorContext(rebuild);
        if (request !== contextRequest || epoch !== sessionEpoch) return;
        this.context = context;
      } catch (err) {
        if (request !== contextRequest || epoch !== sessionEpoch) return;
        this.error = String(err);
      } finally {
        if (request === contextRequest && epoch === sessionEpoch) this.loadingContext = false;
      }
    },

    async loadToolHistory() {
      const request = ++toolsRequest;
      const epoch = sessionEpoch;
      this.loadingTools = true;
      try {
        const history = await api.getDirectorToolHistory();
        if (request !== toolsRequest || epoch !== sessionEpoch) return;
        this.toolHistory = history;
      } catch (err) {
        if (request !== toolsRequest || epoch !== sessionEpoch) return;
        this.toolHistory = [];
        this.error = String(err);
      } finally {
        if (request === toolsRequest && epoch === sessionEpoch) this.loadingTools = false;
      }
    },

    async loadMode(silent = false): Promise<DirectorMode | null> {
      if (this.savingMode || this.confirming || this.discarding) return null;
      const request = ++modeRequest;
      const foreground = silent ? 0 : ++modeForegroundRequest;
      const epoch = sessionEpoch;
      if (!silent) this.loadingMode = true;
      try {
        const mode = await api.getDirectorMode();
        if (request !== modeRequest || epoch !== sessionEpoch) return null;
        applyMode(this, mode);
        this.modeError = "";
        return mode;
      } catch (err) {
        if (request === modeRequest && epoch === sessionEpoch) this.modeError = String(err);
        return null;
      } finally {
        if (!silent && foreground === modeForegroundRequest && epoch === sessionEpoch) {
          this.loadingMode = false;
        }
      }
    },

    async setMode(enabled: boolean, intervalRounds?: number): Promise<boolean> {
      if (this.savingMode || this.confirming || this.discarding) return false;
      const request = ++modeRequest;
      const epoch = sessionEpoch;
      this.savingMode = true;
      this.modeError = "";
      try {
        const mode = await api.setDirectorMode(enabled, intervalRounds);
        if (request !== modeRequest || epoch !== sessionEpoch) return false;
        applyMode(this, mode);
        if (!enabled) this.applyResult = null;
        return true;
      } catch (err) {
        if (request === modeRequest && epoch === sessionEpoch) this.modeError = String(err);
        return false;
      } finally {
        if (epoch === sessionEpoch) this.savingMode = false;
      }
    },

    async confirmDraft() {
      if (this.confirming || this.discarding || this.draft?.status !== "pending") return;
      const request = ++modeRequest;
      const epoch = sessionEpoch;
      this.confirming = true;
      this.applyResult = null;
      this.modeError = "";
      this.draft = { ...this.draft, status: "applying" };
      try {
        const result = await api.confirmDirectorDraft();
        if (request !== modeRequest || epoch !== sessionEpoch) return;
        this.applyResult = result;
        this.draft = result.ok
          ? null
          : this.draft
            ? { ...this.draft, status: "failed", error: result.error }
            : null;
      } catch (err) {
        if (request !== modeRequest || epoch !== sessionEpoch) return;
        this.applyResult = {
          ok: false,
          applied: 0,
          error: `确认结果未知：${String(err)}。请刷新确认状态，或丢弃当前草案`,
        };
      } finally {
        if (epoch === sessionEpoch && request === modeRequest) this.confirming = false;
      }
      if (epoch !== sessionEpoch || request !== modeRequest) return;
      const mode = await this.loadMode(true);
      if (!mode && this.draft?.status === "applying") {
        const message = "确认结果未知，且状态刷新失败。请稍后刷新，或丢弃当前草案";
        this.applyResult = { ok: false, applied: 0, error: message };
        this.draft = { ...this.draft, status: "failed", error: message };
      }
    },

    async discardDraft() {
      if (this.confirming || this.discarding || !this.draft) return;
      const request = ++modeRequest;
      const epoch = sessionEpoch;
      const previous = this.draft;
      this.discarding = true;
      this.modeError = "";
      try {
        const draft = await api.discardDirectorDraft();
        if (request !== modeRequest || epoch !== sessionEpoch) return;
        this.draft = draft;
        this.applyResult = null;
      } catch (err) {
        if (request !== modeRequest || epoch !== sessionEpoch) return;
        this.draft = previous;
        this.modeError = String(err);
      } finally {
        if (epoch === sessionEpoch) this.discarding = false;
      }
    },
  },
});
