/**
 * 剧本编辑器 AI 助手（Skill Agent）store —— 状态定义（setup 风格）。
 *
 * 对话与设置在 DB/后端持久化，前端只做会话内缓存，不进 localStorage。
 */
import { ref } from 'vue'
import type {
  AgentDefaultDirs,
  AgentSettings,
  ConversationInfo,
  SkillInfo,
} from '@/api/services/agent'

/** 工具调用状态机。 */
export type ToolStatus = 'running' | 'pending' | 'done' | 'error' | 'denied'

export interface ToolRun {
  /** 后端生成的 call id，用于匹配 tool_result。 */
  callId: string
  tool: string
  args: Record<string, unknown>
  status: ToolStatus
  output?: string
  requestId?: string
  /** LLM 返回的原始参数 JSON（可能被截断/非法）。 */
  rawArgs?: string
}

/** 一条 assistant 回复的片段：流式文本 + 随后的工具调用。 */
export interface ChatRound {
  content: string
  toolRuns: ToolRun[]
}

export interface ChatItem {
  id: string
  role: 'user' | 'assistant'
  content: string
  rounds: ChatRound[]
  streaming: boolean
  status?: string
  error?: string
}

export interface TokenUsage {
  prompt_tokens: number
  completion_tokens: number
  total_tokens: number
}

export const emptySettings = (): AgentSettings => ({
  providerId: null,
  sandboxDir: null,
  autoApproveCommands: false,
  allowAnyPath: false,
  maxToolRounds: 20,
  systemPrompt: null,
})

export function useAgentState() {
  const conversations = ref<ConversationInfo[]>([])
  const currentId = ref<number | null>(null)
  const items = ref<ChatItem[]>([])
  const streaming = ref(false)
  const sending = ref(false)
  const status = ref('')
  /** 每次事件自增，供视图触发滚动。 */
  const version = ref(0)
  const lastUsage = ref<TokenUsage | null>(null)
  const totalTokens = ref(0)
  const settings = ref<AgentSettings>(emptySettings())
  const skills = ref<SkillInfo[]>([])
  const defaultDirs = ref<AgentDefaultDirs | null>(null)
  const loading = ref(false)

  return {
    conversations,
    currentId,
    items,
    streaming,
    sending,
    status,
    version,
    lastUsage,
    totalTokens,
    settings,
    skills,
    defaultDirs,
    loading,
  }
}
