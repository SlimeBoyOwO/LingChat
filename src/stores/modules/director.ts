/**
 * 导演 Agent store（主聊天页右侧侧栏）。
 *
 * 只有界面偏好（开合 / 宽度 / 当前 tab）进 localStorage；消息、上下文快照、
 * 工具历史都是后端运行时状态，持久化插件每次 mutation 全量写入，必须排除。
 */
import { Channel, invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import * as api from "@/api/services/director";
import type { DirectorAgentEvent, DirectorContext, DirectorMessage } from "@/api/services/director";

export type DirectorTab = "chat" | "context" | "tools";

/** 面板宽度限制：默认 420，最小 320，最大取 min(720, 55vw) 由视图侧钳制。 */
export const DIRECTOR_DEFAULT_WIDTH = 420;
export const DIRECTOR_MIN_WIDTH = 320;
export const DIRECTOR_MAX_WIDTH = 720;

/** 本轮正在流式产出的回复（Done 后转成 messages 里的一条）。 */
export interface DirectorStreaming {
  content: string;
  reasoning: string;
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
  status: string;
  error: string;
  context: DirectorContext | null;
  loadingContext: boolean;
  toolHistory: DirectorMessage[];
  loadingTools: boolean;
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
    status: "",
    error: "",
    context: null,
    loadingContext: false,
    toolHistory: [],
    loadingTools: false,
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
      "status",
      "error",
      "context",
      "loadingContext",
      "toolHistory",
      "loadingTools",
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
        // 工具事件本期只保留在事件协议里，由「工具调用」tab 从后端历史读取；
        // 步骤二接入工具后在此追加轮内展示
        case "tool_call":
        case "tool_result":
          break;
        case "done":
          this.finish(event.final_text, event.message_id);
          break;
        case "error":
          this.fail(event.message);
          break;
      }
    },

    /** 正常收尾：把流式产出转成一条 assistant 消息。 */
    finish(finalText: string, messageId: number) {
      const s = this.streaming;
      this.streaming = null;
      this.running = false;
      this.status = "";
      runSeq++;
      if (!s) return;
      const content = s.content || finalText;
      const reasoning = s.reasoning || null;
      if (!content && !reasoning) return;
      this.messages.push(
        emptyMessage("assistant", content, {
          id: messageId > 0 ? messageId : 0,
          reasoning,
        }),
      );
    },

    /** 出错收尾：保留已经流给玩家看的内容，错误行显示在消息列表末尾。 */
    fail(message: string) {
      const s = this.streaming;
      this.error = message;
      this.streaming = null;
      this.running = false;
      this.status = "";
      runSeq++;
      if (s && (s.content || s.reasoning)) {
        this.messages.push(
          emptyMessage("assistant", s.content, { reasoning: s.reasoning || null }),
        );
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
  },
});
