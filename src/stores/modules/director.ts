/**
 * 导演 Agent store（主聊天页右侧侧栏）。
 *
 * 只有界面偏好（开合 / 宽度 / 当前 tab）进 localStorage；消息、上下文快照、
 * 工具历史都是后端运行时状态，持久化插件每次 mutation 全量写入，必须排除。
 */
import { Channel, invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import * as api from "@/api/services/director";
import type {
  DirectorAgentEvent,
  DirectorConfirmResult,
  DirectorContext,
  DirectorDraft,
  DirectorMessage,
} from "@/api/services/director";

export type DirectorTab = "chat" | "context" | "tools" | "draft";

/** 面板宽度限制：默认 420，最小 320，最大取 min(720, 55vw) 由视图侧钳制。 */
export const DIRECTOR_DEFAULT_WIDTH = 420;
export const DIRECTOR_MIN_WIDTH = 320;
export const DIRECTOR_MAX_WIDTH = 720;

/** 本轮正在流式产出的回复（Done 后转成 messages 里的一条）。 */
export interface DirectorStreaming {
  content: string;
  reasoning: string;
}

/** 本轮一次工具调用的实时状态（收尾后归档进消息并清空，不持久化）。 */
export interface DirectorActiveTool {
  tool: string;
  args: Record<string, unknown>;
  /** pending 为预留：事件驱动下 tool_call 一到就是 running */
  status: "pending" | "running" | "ok" | "error";
  output: string | null;
  error: string | null;
}

/** 周期小剧本的导演模式（只留界面要用的三个字段，pendingDraft 单独放 draft）。 */
export interface DirectorModeState {
  enabled: boolean;
  interval: number;
  lastGeneratedSeq: number | null;
}

export interface DirectorStoreState {
  /** 界面偏好 */
  open: boolean;
  width: number;
  activeTab: DirectorTab;

  /** 运行时状态（不持久化） */
  ready: boolean;
  available: boolean;
  saveBound: boolean;
  saveId: number | null;
  /** 当前存档标题（只说给界面看，后端状态里只有 id） */
  saveTitle: string | null;
  running: boolean;
  messages: DirectorMessage[];
  streaming: DirectorStreaming | null;
  /** 本轮进行中的工具调用（call_id → 实时状态；done/error 后归档进消息并清空） */
  activeTools: Record<string, DirectorActiveTool>;
  status: string;
  error: string;
  context: DirectorContext | null;
  loadingContext: boolean;
  toolHistory: DirectorMessage[];
  loadingTools: boolean;

  /** 周期小剧本运行态（不持久化） */
  directorMode: DirectorModeState;
  draft: DirectorDraft | null;
  loadingMode: boolean;
  confirming: boolean;
  applyResult: DirectorConfirmResult | null;
}

/** 本轮序号：停止 / 完成 / 出错后自增，迟到的流式事件按序号丢弃。 */
let runSeq = 0;

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

/** 工具结果由 ToolExecutor 统一编码成 {"ok":false,...}；非 JSON 文本视为成功 */
export function isDirectorToolResultOk(content: string): boolean {
  try {
    const parsed = JSON.parse(content) as { ok?: boolean };
    return parsed.ok !== false;
  } catch {
    return true;
  }
}

/** 工具参数的单行摘要：截断到 limit 字符，完整内容由调用方放 title */
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
    confirming: false,
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
      "confirming",
      "applyResult",
    ],
  },

  getters: {
    /** 状态点：不可用 / 生成中 / 空闲。 */
    statusKind: (state): "unavailable" | "running" | "idle" => {
      if (!state.available) return "unavailable";
      return state.running ? "running" : "idle";
    },
  },

  actions: {
    /** 首次挂载拉状态；已就绪后不重复拉，避免抹掉进行中的流式内容。 */
    async init() {
      if (this.ready) return;
      await this.loadState();
    },

    async loadState() {
      try {
        const state = await api.getDirectorState();
        this.ready = true;
        this.available = state.available;
        this.saveBound = state.saveBound;
        this.saveId = state.saveId;
        this.running = state.running;
        // 流式中不回灌历史：本地已流出的正文比后端落库状态新
        if (!this.streaming) this.messages = state.messages;
        this.error = "";
      } catch (err) {
        // 后端未接入时命令不存在，这里只记下原因，由视图给空态
        this.ready = false;
        this.available = false;
        this.error = String(err);
        return;
      }
      await this.loadSaveTitle();
    },

    /** 存档标题只用于标题栏展示，查不到时由视图退回存档编号 */
    async loadSaveTitle() {
      const saveId = this.saveId;
      this.saveTitle = null;
      if (saveId == null) return;
      try {
        const res = await invoke<{ saves: { id: number; title: string }[] }>("list_saves", {
          page: 1,
          pageSize: 50,
        });
        // 期间可能又切了档，只认当前存档
        if (this.saveId === saveId) {
          this.saveTitle = res.saves.find((s) => s.id === saveId)?.title ?? null;
        }
      } catch (err) {
        console.warn("[Director] 读取存档名失败:", err);
      }
    },

    async send(text: string) {
      const content = text.trim();
      if (!content || this.running) return;

      this.error = "";
      this.status = "";
      this.running = true;
      this.activeTools = {};
      this.messages.push(emptyMessage("user", content));
      this.streaming = { content: "", reasoning: "" };

      const seq = ++runSeq;
      const channel = new Channel<DirectorAgentEvent>();
      channel.onmessage = (event) => {
        if (seq !== runSeq) return;
        this.handleEvent(event);
      };

      try {
        const id = await api.startDirectorChat(content, channel);
        // 后端返回本条用户消息的 DB id（0 = 自由对话未落库）
        const last = this.messages[this.messages.length - 1];
        if (id > 0 && last && last.role === "user" && last.id === 0) last.id = id;
      } catch (err) {
        if (seq !== runSeq) return;
        this.fail(String(err));
      }
    },

    handleEvent(event: DirectorAgentEvent) {
      const s = this.streaming;
      switch (event.type) {
        case "status":
          this.status = event.content;
          break;
        case "message_delta":
          if (s) s.content += event.content;
          break;
        case "reasoning":
          if (s) s.reasoning += event.content;
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
          // 结果按 call_id 回填；迟到的孤儿结果直接忽略
          const entry = this.activeTools[event.call_id];
          if (!entry) break;
          entry.status = event.ok ? "ok" : "error";
          entry.output = event.output;
          entry.error = event.error;
          break;
        }
        case "done":
          this.finish(event.final_text, event.message_id);
          break;
        case "error":
          this.fail(event.message);
          break;
      }
    },

    /** 正常收尾：把流式产出转成一条 assistant 消息，并归档本轮工具调用。 */
    finish(finalText: string, messageId: number) {
      const s = this.streaming;
      this.streaming = null;
      this.running = false;
      this.status = "";
      runSeq++;
      let pushed: DirectorMessage | null = null;
      if (s) {
        const content = s.content || finalText;
        const reasoning = s.reasoning || null;
        if (content || reasoning) {
          pushed = emptyMessage("assistant", content, {
            id: messageId > 0 ? messageId : 0,
            reasoning,
          });
          this.messages.push(pushed);
        }
      }
      this.archiveActiveTools(pushed);
    },

    /** 出错收尾：保留已经流给玩家看的内容，错误行显示在消息列表末尾。 */
    fail(message: string) {
      const s = this.streaming;
      this.error = message;
      this.streaming = null;
      this.running = false;
      this.status = "";
      runSeq++;
      let pushed: DirectorMessage | null = null;
      if (s && (s.content || s.reasoning)) {
        pushed = emptyMessage("assistant", s.content, { reasoning: s.reasoning || null });
        this.messages.push(pushed);
      }
      this.archiveActiveTools(pushed);
    },

    /**
     * 收尾归档：清空 activeTools，本轮调用挂到本轮 assistant 消息（pushed）
     * 的 toolCalls 上；本轮没产出正文时补一条空正文载体，避免误挂上一轮。
     * 有结果的再补 role=tool 消息，结构与后端落库一致。
     */
    archiveActiveTools(pushed: DirectorMessage | null) {
      const archived = Object.entries(this.activeTools);
      this.activeTools = {};
      if (archived.length === 0) return;
      const last = pushed ?? emptyMessage("assistant", "");
      if (!pushed) this.messages.push(last);
      const calls = archived.map(([id, entry]) => ({
        id,
        type: "function",
        function: { name: entry.tool, arguments: JSON.stringify(entry.args ?? {}) },
      }));
      last.toolCalls = [...(last.toolCalls ?? []), ...calls];
      for (const [id, entry] of archived) {
        // 中断等场景会留下 running 条目：不落结果消息，展示为「无结果」
        if (entry.status !== "ok" && entry.status !== "error") continue;
        const content =
          entry.status === "error" ? entry.error || entry.output || "" : entry.output || "";
        this.messages.push(emptyMessage("tool", content, { toolCallId: id }));
      }
    },

    async stop() {
      if (!this.running) return;
      // 先本地收尾（界面立刻可继续输入），再让后端把半截回复落库后回灌
      this.finish("", 0);
      try {
        await api.stopDirectorChat();
        await this.loadState();
      } catch (err) {
        console.warn("[Director] 停止请求失败:", err);
      }
    },

    async clear() {
      if (this.running) return;
      try {
        await api.clearDirector();
      } catch (err) {
        this.error = String(err);
        return;
      }
      this.messages = [];
      this.activeTools = {};
      this.context = null;
      this.toolHistory = [];
      this.error = "";
    },

    /** 「查看记忆」：默认展示上次实际请求的快照，rebuild 时按当前世界状态重算。 */
    async loadContext(rebuild = false) {
      this.loadingContext = true;
      try {
        this.context = await api.getDirectorContext(rebuild);
        this.error = "";
      } catch (err) {
        this.context = null;
        this.error = String(err);
      } finally {
        this.loadingContext = false;
      }
    },

    async loadToolHistory() {
      this.loadingTools = true;
      try {
        this.toolHistory = await api.getDirectorToolHistory();
      } catch (err) {
        this.toolHistory = [];
        this.error = String(err);
      } finally {
        this.loadingTools = false;
      }
    },

    /** 拉取导演模式；pendingDraft 顺带作为草案初值，避免再多一次往返。 */
    async loadMode() {
      this.loadingMode = true;
      try {
        const mode = await api.getDirectorMode();
        this.directorMode = {
          enabled: mode.enabled,
          interval: mode.interval,
          lastGeneratedSeq: mode.lastGeneratedSeq,
        };
        // pendingDraft 是草案对象或 null；只接对象/空值，避免异常值污染运行态
        const pending = mode.pendingDraft;
        if (pending === null || typeof pending === "object") this.draft = pending;
      } catch {
        // 后端未接入时命令不存在，保持默认空态，不弹错
      } finally {
        this.loadingMode = false;
      }
    },

    async setMode(enabled: boolean, intervalRounds?: number) {
      try {
        await api.setDirectorMode(enabled, intervalRounds);
      } catch {
        // 吞掉错误，随后用 loadMode 回读真实状态
      }
      await this.loadMode();
    },

    async loadDraft() {
      try {
        this.draft = await api.getDirectorDraft();
      } catch {
        // 保留现有草案，避免轮询时闪烁；后端未就绪时本就是空态
      }
    },

    async confirmDraft() {
      if (this.confirming) return;
      this.confirming = true;
      this.applyResult = null;
      try {
        const res = await api.confirmDirectorDraft();
        this.applyResult = res;
        if (res.ok) {
          this.draft = null;
          await this.loadMode();
        }
      } catch (err) {
        this.applyResult = { ok: false, applied: 0, error: String(err) };
      } finally {
        this.confirming = false;
      }
    },

    async discardDraft() {
      try {
        await api.discardDirectorDraft();
      } catch {
        // 吞掉错误，本地照常清掉草案
      }
      this.draft = null;
      this.applyResult = null;
    },
  },
});
