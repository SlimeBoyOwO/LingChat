/**
 * 剧本编辑器 AI 助手 store —— actions。
 *
 * 事件处理复刻 ling_chat_agent `chat.ts` 的 round 分段 / 工具挂载逻辑；
 * 差异：历史由后端从 DB 重建，前端只传本次消息。
 */
import { Channel } from '@tauri-apps/api/core'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import { useUIStore } from '@/stores/modules/ui/ui'
import * as api from '@/api/services/agent'
import type { PersistedMessage, SkillAgentEvent } from '@/api/services/agent'
import { useAgentState } from './state'
import type { ChatItem, ChatRound, TokenUsage, ToolRun } from './state'

let idCounter = 0
const nextId = () => `m-${Date.now()}-${++idCounter}`

/** 当前正在生成的 assistant 消息 id；跨 turn 复位。 */
let activeAssistantId: string | null = null
/** 当前 turn 的流式通道；turn 结束后置空。 */
let channel: Channel<SkillAgentEvent> | null = null

const safeParse = (s: string): Record<string, unknown> => {
  try {
    return JSON.parse(s) as Record<string, unknown>
  } catch {
    return {}
  }
}

const itemText = (m: ChatItem): string =>
  m.role === 'user' ? m.content : m.rounds.map((r) => r.content).join('')

export function useAgentActions(state: ReturnType<typeof useAgentState>) {
  const scriptEditor = useScriptEditorStore()
  const uiStore = useUIStore()

  // ==================== 会话 ====================

  /** 进入面板时初始化：拉设置/技能/会话，自动建会话并绑定当前剧本 key。 */
  async function initForEditor() {
    state.loading.value = true
    try {
      state.settings.value = await api.getAgentSettings()
      state.skills.value = await api.listAgentSkills()
      state.defaultDirs.value = await api.getAgentDefaultDirs()
      state.conversations.value = await api.listAgentConversations()
      if (state.conversations.value.length === 0) {
        await createConversation()
      } else {
        // 自动切到最近更新的会话
        await switchConversation(state.conversations.value[0].id)
      }
    } finally {
      state.loading.value = false
    }
  }

  async function createConversation() {
    if (state.streaming.value) await cancel()
    const key = scriptEditor.scriptKey ?? null
    const conv = await api.createAgentConversation(key)
    state.conversations.value.unshift(conv)
    await switchConversation(conv.id)
    return conv
  }

  async function switchConversation(id: number) {
    if (state.streaming.value) await cancel()
    state.currentId.value = id
    const msgs = await api.getAgentMessages(id)
    state.items.value = rebuildItems(msgs)
    state.status.value = ''
    state.version.value++
  }

  async function deleteConversation(id: number) {
    await api.deleteAgentConversation(id)
    state.conversations.value = state.conversations.value.filter((c) => c.id !== id)
    if (state.currentId.value === id) {
      state.currentId.value = null
      state.items.value = []
      if (state.conversations.value.length > 0) {
        await switchConversation(state.conversations.value[0].id)
      } else {
        await createConversation()
      }
    }
  }

  async function clearConversation() {
    if (state.currentId.value == null) return
    await api.clearAgentConversation(state.currentId.value)
    state.items.value = []
    state.version.value++
  }

  // ==================== 对话 ====================

  async function sendMessage(text: string) {
    const content = text.trim()
    if (!content || state.streaming.value || state.currentId.value == null) return

    state.items.value.push({
      id: nextId(),
      role: 'user',
      content,
      rounds: [],
      streaming: false,
    })
    activeAssistantId = nextId()
    state.items.value.push({
      id: activeAssistantId,
      role: 'assistant',
      content: '',
      rounds: [],
      streaming: true,
    })

    state.streaming.value = true
    state.sending.value = true
    state.status.value = '思考中…'
    state.version.value++

    channel = new Channel<SkillAgentEvent>()
    channel.onmessage = (event: SkillAgentEvent) => handleEvent(event)

    try {
      await api.startAgentChat(state.currentId.value, content, channel)
    } catch (err) {
      finishWithError(String(err))
    }
  }

  function handleEvent(event: SkillAgentEvent) {
    const msg = currentAssistant()
    switch (event.type) {
      case 'status':
        state.status.value = event.content
        if (msg) msg.status = event.content
        break
      case 'message_delta': {
        if (!msg) break
        const last = msg.rounds[msg.rounds.length - 1]
        if (last && last.toolRuns.length === 0) {
          last.content += event.content
        } else {
          // 上一段以工具调用结尾 → 新开一段
          msg.rounds.push({ content: event.content, toolRuns: [] })
        }
        break
      }
      case 'reasoning':
        // 暂不展示思考链
        break
      case 'tool_call': {
        if (!msg) break
        let round = msg.rounds[msg.rounds.length - 1]
        if (!round) {
          round = { content: '', toolRuns: [] }
          msg.rounds.push(round)
        }
        round.toolRuns.push({
          callId: event.call_id,
          tool: event.tool,
          args: event.args,
          status: 'running',
          rawArgs: event.raw_args,
        })
        state.status.value = `正在调用工具: ${event.tool}`
        break
      }
      case 'pending_approval': {
        state.status.value = '等待你的审批…'
        if (!msg) break
        // 审批紧随对应 tool_call 到达：找最后一条该工具的 running run
        let run: ToolRun | undefined
        for (let i = msg.rounds.length - 1; i >= 0 && !run; i--) {
          run = msg.rounds[i].toolRuns.find(
            (r) => r.tool === event.tool && r.status === 'running',
          )
        }
        if (run) {
          run.status = 'pending'
          run.requestId = event.request_id
        } else {
          const round = msg.rounds[msg.rounds.length - 1] ?? { content: '', toolRuns: [] }
          if (!msg.rounds.includes(round)) msg.rounds.push(round)
          round.toolRuns.push({
            callId: `approval-${event.request_id}`,
            tool: event.tool,
            args: event.args,
            status: 'pending',
            requestId: event.request_id,
          })
        }
        break
      }
      case 'tool_result': {
        const run = msg ? findRun(msg, event.call_id) : undefined
        if (run) {
          run.status =
            !event.ok && event.output.includes('已拒绝')
              ? 'denied'
              : event.ok
                ? 'done'
                : 'error'
          run.output = event.output
        }
        state.status.value = ''
        break
      }
      case 'done':
        finish(activeAssistantId, event.final_text || undefined, event.usage ?? null)
        break
      case 'error':
        finishWithError(event.message)
        break
    }
    state.version.value++
  }

  function currentAssistant(): ChatItem | undefined {
    return state.items.value.find((m) => m.id === activeAssistantId)
  }

  function findRun(msg: ChatItem, callId: string): ToolRun | undefined {
    for (const round of msg.rounds) {
      const run = round.toolRuns.find((r) => r.callId === callId)
      if (run) return run
    }
    return undefined
  }

  function finish(
    assistantId: string | null,
    finalText?: string,
    usage?: TokenUsage | null,
  ) {
    const msg = state.items.value.find((m) => m.id === assistantId)
    if (msg) {
      msg.streaming = false
      // 只有完全没流式过时才用最终文本填段
      if (finalText && finalText.length > 0 && itemText(msg).length === 0) {
        if (msg.rounds.length === 0) msg.rounds.push({ content: '', toolRuns: [] })
        msg.rounds[msg.rounds.length - 1].content = finalText
      }
    }
    if (usage) {
      state.lastUsage.value = usage
      state.totalTokens.value += usage.total_tokens
    }
    state.streaming.value = false
    state.sending.value = false
    state.status.value = ''
    activeAssistantId = null
    channel = null
    state.version.value++
  }

  function finishWithError(message: string) {
    const msg = currentAssistant()
    if (msg) {
      msg.streaming = false
      msg.error = message
    }
    state.streaming.value = false
    state.sending.value = false
    state.status.value = ''
    activeAssistantId = null
    channel = null
    state.version.value++
  }

  async function cancel() {
    if (!state.streaming.value) return
    await api.stopAgentChat()
    finish(activeAssistantId)
  }

  async function resolveApproval(requestId: string, allowed: boolean) {
    await api.resolveAgentApproval(requestId, allowed)
    const msg = currentAssistant()
    const run = msg?.rounds
      .flatMap((r) => r.toolRuns)
      .find((r) => r.requestId === requestId)
    if (run) {
      run.status = allowed ? 'running' : 'denied'
      if (!allowed) run.output = '已拒绝执行'
    }
    state.version.value++
  }

  // ==================== 设置 ====================

  async function loadSettings() {
    state.settings.value = await api.getAgentSettings()
  }

  async function loadSkills() {
    state.skills.value = await api.listAgentSkills()
  }

  async function saveSettings() {
    await api.saveAgentSettings(state.settings.value)
    uiStore.showNotification({
      type: 'success',
      title: '设置已保存',
      message: '剧本导师设置已保存，下次对话生效。',
      skipTipsCheck: true,
    })
  }

  // ==================== 历史重建 ====================

  /** 把后端返回的消息重建成 UI 的 ChatItem（assistant 按 tool_calls 拆 round，tool 结果挂回对应 run）。 */
  function rebuildItems(msgs: PersistedMessage[]): ChatItem[] {
    const items: ChatItem[] = []
    let current: ChatItem | null = null
    for (const m of msgs) {
      if (m.role === 'user') {
        current = null
        items.push({
          id: `p-${m.id}`,
          role: 'user',
          content: m.content ?? '',
          rounds: [],
          streaming: false,
        })
      } else if (m.role === 'assistant') {
        const round: ChatRound = {
          content: m.content ?? '',
          toolRuns: (m.toolCalls ?? []).map((tc) => ({
            callId: tc.id,
            tool: tc.function.name,
            args: safeParse(tc.function.arguments),
            status: 'done' as const,
            rawArgs: tc.function.arguments,
            output: '（工具已执行，结果见下方）',
          })),
        }
        if (current && current.role === 'assistant') {
          current.rounds.push(round)
        } else {
          current = {
            id: `p-${m.id}`,
            role: 'assistant',
            content: '',
            rounds: [round],
            streaming: false,
          }
          items.push(current)
        }
      } else if (m.role === 'tool') {
        const run = current?.rounds
          .flatMap((r) => r.toolRuns)
          .find((r) => r.callId === m.toolCallId)
        if (run) run.output = m.content ?? ''
      }
    }
    return items
  }

  return {
    initForEditor,
    createConversation,
    switchConversation,
    deleteConversation,
    clearConversation,
    sendMessage,
    cancel,
    resolveApproval,
    loadSettings,
    loadSkills,
    saveSettings,
  }
}

export type AgentActions = ReturnType<typeof useAgentActions>
