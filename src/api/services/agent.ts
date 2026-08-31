import { invoke, type Channel } from "@tauri-apps/api/core";

/**
 * 剧本编辑器 AI 助手（Skill Agent）的后端接口。
 *
 * 只做 invoke 封装，不含业务逻辑。命令均为 `editor_agent_*`（见
 * `src-tauri/src/api/script_editor/agent.rs`）。
 */

// ============================================================
// DTO
// ============================================================

/** Agent 设置（沙箱目录为空表示默认 data/）。 */
export interface AgentSettings {
  providerId: string | null;
  sandboxDir: string | null;
  autoApproveCommands: boolean;
  allowAnyPath: boolean;
  maxToolRounds: number;
  systemPrompt: string | null;
  /** 思考模式覆盖；null 表示跟随模型 provider 默认（独立于主对话 LLM 设置）。 */
  enableThinking: boolean | null;
}

/** 发现的技能。 */
export interface SkillInfo {
  name: string;
  description: string;
  location: string;
  path: string;
}

/** 技能 SKILL.md 内容。 */
export interface SkillContent {
  name: string;
  baseDirectory: string;
  content: string;
}

/** 一个对话会话。 */
export interface ConversationInfo {
  id: number;
  title: string | null;
  /** 创建会话时打开的剧本 key（可为 null）。 */
  scriptKey: string | null;
  createdAt: string;
  updatedAt: string;
}

/** 持久化消息（OpenAI 格式）。 */
export interface PersistedMessage {
  id: number;
  role: "user" | "assistant" | "tool" | "system";
  content: string | null;
  /** assistant 的思考链（仅展示，不参与 LLM 上下文；旧数据为 null）。 */
  reasoning: string | null;
  /** 产生该消息那一轮 LLM 调用的 token 用量（仅 assistant 消息有；旧数据为 null）。 */
  promptTokens: number | null;
  completionTokens: number | null;
  /** 输入中命中缓存（cache read）的 token 数（旧数据为 null）。 */
  cachedTokens: number | null;
  toolCalls:
    | {
        id: string;
        type: string;
        function: { name: string; arguments: string };
      }[]
    | null;
  toolCallId: string | null;
  createdAt: string;
}

/** 设置面板展示的默认目录。 */
export interface AgentDefaultDirs {
  dataDir: string;
  skillsDir: string;
  sandboxDir: string;
}

/** Token 用量。 */
export interface TokenUsage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  /** 输入中命中缓存（cache read）的 token 数；未上报时为 0。 */
  cached_tokens: number;
}

/** 后端流式事件（serde tag="type"，snake_case）。 */
export type SkillAgentEvent =
  | { type: "status"; content: string }
  | { type: "message_delta"; content: string }
  | { type: "reasoning"; content: string }
  | {
      type: "tool_call";
      call_id: string;
      tool: string;
      args: Record<string, unknown>;
      raw_args: string;
    }
  | {
      type: "tool_result";
      call_id: string;
      tool: string;
      ok: boolean;
      output: string;
      error: string | null;
    }
  | {
      type: "pending_approval";
      request_id: string;
      tool: string;
      args: Record<string, unknown>;
    }
  | { type: "done"; final_text: string; usage: TokenUsage | null }
  | { type: "conversation_title"; title: string }
  | { type: "error"; message: string };

// ============================================================
// 设置
// ============================================================

export const getAgentSettings = () => invoke<AgentSettings>("editor_agent_get_settings");

export const saveAgentSettings = (settings: AgentSettings) =>
  invoke<void>("editor_agent_save_settings", { settings });

export const getAgentDefaultDirs = () => invoke<AgentDefaultDirs>("editor_agent_get_default_dirs");

// ============================================================
// 技能
// ============================================================

export const listAgentSkills = () => invoke<SkillInfo[]>("editor_agent_list_skills");

export const readAgentSkill = (name: string) =>
  invoke<SkillContent>("editor_agent_read_skill", { name });

// ============================================================
// 会话
// ============================================================

export const createAgentConversation = (scriptKey: string | null) =>
  invoke<ConversationInfo>("editor_agent_create_conversation", { scriptKey });

export const listAgentConversations = () =>
  invoke<ConversationInfo[]>("editor_agent_list_conversations");

export const deleteAgentConversation = (conversationId: number) =>
  invoke<void>("editor_agent_delete_conversation", { conversationId });

/** 重命名会话（用户自定义标题；标题非空后不再自动生成）。 */
export const renameAgentConversation = (conversationId: number, title: string) =>
  invoke<void>("editor_agent_rename_conversation", { conversationId, title });

export const getAgentMessages = (conversationId: number) =>
  invoke<PersistedMessage[]>("editor_agent_get_messages", { conversationId });

export const clearAgentConversation = (conversationId: number) =>
  invoke<void>("editor_agent_clear_conversation", { conversationId });

// ============================================================
// 对话
// ============================================================

/** 开始一轮对话。返回本次用户消息的 DB id（用于回溯删除定位）。 */
export const startAgentChat = (
  conversationId: number,
  message: string,
  channel: Channel<SkillAgentEvent>
) => invoke<number>("editor_agent_start_chat", { conversationId, message, channel });

export const stopAgentChat = () => invoke<void>("editor_agent_stop_chat");

/** 回溯：删除会话中 id >= messageId 的消息（把对话回退到该消息发送前）。 */
export const rewindAgentMessages = (conversationId: number, messageId: number) =>
  invoke<void>("editor_agent_rewind", { conversationId, messageId });

export const resolveAgentApproval = (requestId: string, allowed: boolean) =>
  invoke<void>("editor_agent_resolve_approval", { requestId, allowed });
