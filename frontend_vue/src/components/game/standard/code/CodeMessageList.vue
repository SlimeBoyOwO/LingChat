<template>
  <div ref="scrollRef" class="code-assistant-body">
    <!-- 空状态欢迎消息 -->
    <div v-if="props.messages.length === 0" class="assistant-message">
      <div class="assistant-avatar"><Bot :size="18" /></div>
      <div class="assistant-bubble">
        <div class="assistant-text">{{ props.welcomeMessage }}</div>
        <div class="assistant-text muted">
          Code 模式已准备好，可以问我代码问题，或者让我继续执行任务。
        </div>
      </div>
    </div>

    <!-- 对话消息 -->
    <article
      v-for="(message, index) in props.messages"
      :key="messageKey(message, index)"
      :class="message.type === 'message' ? 'user-message' : 'assistant-message'"
    >
      <template v-if="message.type === 'reply'">
        <div class="assistant-avatar"><Bot :size="18" /></div>
        <div class="assistant-bubble">
          <div class="assistant-text">{{ messageText(message, index) }}</div>
          <time v-if="message.timestamp" class="message-time">{{
            formatTime(message.timestamp)
          }}</time>
        </div>
      </template>

      <template v-else>
        <div class="user-bubble">
          <div>{{ message.content }}</div>
          <time v-if="message.timestamp" class="message-time">{{
            formatTime(message.timestamp)
          }}</time>
        </div>
      </template>
    </article>

    <!-- 工具调用日志 -->
    <article
      v-for="tool in props.toolLogs"
      :key="toolKey(tool)"
      class="assistant-message tool-event-message"
    >
      <div class="assistant-avatar tool-avatar">
        <LoaderCircle v-if="tool.status === 'running'" :size="16" class="animate-spin" />
        <CheckCircle2 v-else-if="tool.ok !== false && tool.status !== 'error'" :size="16" />
        <Wrench v-else :size="16" />
      </div>
      <div class="assistant-bubble tool-event-bubble" :class="toolStatusClass(tool)">
        <div class="tool-event-title">{{ toolSummary(tool) }}</div>
        <div class="tool-event-meta">
          <span>{{ tool.tool }}</span>
          <span>{{ toolStatusText(tool) }}</span>
          <span>{{ formatToolTime(tool.timestamp) }}</span>
        </div>
        <div v-if="toolDetail(tool)" class="tool-event-detail">{{ toolDetail(tool) }}</div>
      </div>
    </article>

    <!-- 工具状态浮标 -->
    <article v-if="props.showStatusBubble" class="assistant-message">
      <div class="assistant-avatar"><Wrench :size="17" /></div>
      <div class="assistant-bubble tool-bubble">
        <div class="tool-status">
          <LoaderCircle v-if="props.isThinking" :size="14" class="animate-spin" />
          <CheckCircle2 v-else :size="14" />
          <span>{{ props.statusBubbleText }}</span>
        </div>
        <div v-if="props.toolPreview" class="tool-preview">{{ props.toolPreview }}</div>
      </div>
    </article>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onBeforeUnmount } from 'vue'
import { Bot, CheckCircle2, LoaderCircle, Wrench } from 'lucide-vue-next'
import type { GameMessage } from '@/stores/modules/game/state'
import type { ToolCallLog } from '@/stores/modules/ui/ui'

const props = withDefaults(defineProps<{
  messages: GameMessage[]
  toolLogs: ToolCallLog[]
  showStatusBubble: boolean
  statusBubbleText: string
  toolPreview: string
  isThinking: boolean
  welcomeMessage?: string
}>(), {
  welcomeMessage: '晚上好，主人！',
})

// ── 打字效果 ──

const typingKey = ref('')
const typedText = ref('')
let typingTimer: number | null = null

const scrollRef = ref<HTMLElement | null>(null)

const messageKey = (message: GameMessage, index: number) => {
  return `${message.timestamp || index}-${message.type}-${message.displayName}-${message.content.slice(0, 16)}`
}

const messageText = (message: GameMessage, index: number) => {
  const key = messageKey(message, index)
  return key === typingKey.value ? typedText.value : message.content
}

// 监听最新的 reply，触发打字效果
const latestReply = computed(() => {
  for (let i = props.messages.length - 1; i >= 0; i--) {
    const m = props.messages[i]
    if (m?.type === 'reply') return { key: messageKey(m, i), content: m.content || '' }
  }
  return null
})

watch(latestReply, (cur, prev) => {
  if (cur && cur.key !== prev?.key) {
    startTyping(cur.key, cur.content)
  }
})

const stopTyping = () => {
  if (typingTimer !== null) {
    window.clearInterval(typingTimer)
    typingTimer = null
  }
}

const startTyping = (key: string, content: string) => {
  stopTyping()
  typingKey.value = key
  typedText.value = ''
  let cursor = 0
  const step = () => {
    cursor += content.charCodeAt(cursor) > 255 ? 1 : 2
    typedText.value = content.slice(0, cursor)
    scrollToBottom()
    if (cursor >= content.length) {
      stopTyping()
      typedText.value = content
    }
  }
  typingTimer = window.setInterval(step, 18)
}

// ── 自动滚动 ──

const scrollToBottom = () => {
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      if (scrollRef.value) {
        scrollRef.value.scrollTo({
          top: scrollRef.value.scrollHeight,
          behavior: 'smooth',
        })
      }
    })
  })
}

// 消息数量或状态变化时滚动到底部
watch(
  [() => props.messages.length, () => props.isThinking, () => props.showStatusBubble],
  scrollToBottom,
  { immediate: true },
)

// ── 工具日志相关工具函数 ──

const formatTime = (timestamp: number) => {
  return new Date(timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

const formatToolTime = (timestamp: string) => {
  const date = timestamp ? new Date(timestamp) : new Date()
  return Number.isNaN(date.getTime())
    ? ''
    : date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

const toolKey = (tool: ToolCallLog) => `${tool.id || tool.timestamp}-${tool.tool}-${tool.status}`

// 工具详情渲染器注册表（可按需扩展）
const parseToolPreview = (tool: ToolCallLog): any => {
  if (!tool.preview) return null
  try { return JSON.parse(tool.preview) }
  catch { return null }
}

const nestedToolResult = (tool: ToolCallLog): any => {
  const parsed = parseToolPreview(tool)
  if (!parsed || typeof parsed !== 'object') return null
  return parsed.result && typeof parsed.result === 'object' ? parsed.result : parsed
}

type ToolDetailRenderer = (tool: ToolCallLog) => string

const toolDetailRenderers: Record<string, ToolDetailRenderer> = {
  sandbox_write_file: (tool) => {
    const r = nestedToolResult(tool)
    if (!r || typeof r !== 'object') return ''
    const parts: string[] = []
    if (r.line_count !== undefined) parts.push(`${r.line_count} 行`)
    if (r.bytes !== undefined) parts.push(`${r.bytes} bytes`)
    if (r.syntax_check?.ok) parts.push('语法检查通过')
    if (r.incomplete_warning) parts.push('可能未写完整')
    return parts.join(' · ')
  },
  sandbox_execute_command: (tool) => {
    const r = nestedToolResult(tool)
    if (!r || typeof r !== 'object') return ''
    const output = String(r.stdout || r.stderr || '').trim()
    return output.length > 180 ? `${output.slice(0, 180)}...` : output
  },
  sandbox_list_files: (tool) => {
    const r = nestedToolResult(tool)
    if (!r || typeof r !== 'object' || !Array.isArray(r.items)) return ''
    return r.items.slice(0, 4).map((item: any) => item.name).join(' · ')
  },
  sandbox_read_file: (tool) => {
    const r = nestedToolResult(tool)
    if (!r || typeof r !== 'object') return ''
    const size = r.size ?? r.bytes
    return size ? `${Number(size).toLocaleString()} bytes` : ''
  },
}

const toolSummary = (tool: ToolCallLog) => {
  if (tool.summary) return tool.summary
  if (tool.status === 'running') return `正在执行 ${tool.tool}`
  if (tool.status === 'error' || tool.ok === false) return `${tool.tool} 执行失败`
  return `${tool.tool} 执行完成`
}

const toolStatusText = (tool: ToolCallLog) => {
  if (tool.status === 'running') return '运行中'
  if (tool.status === 'error' || tool.ok === false) return '失败'
  return '完成'
}

const toolStatusClass = (tool: ToolCallLog) => ({
  'tool-running': tool.status === 'running',
  'tool-success': tool.status !== 'running' && tool.ok !== false && tool.status !== 'error',
  'tool-error': tool.status === 'error' || tool.ok === false,
})

const toolDetail = (tool: ToolCallLog): string => {
  const renderer = toolDetailRenderers[tool.tool]
  return renderer ? renderer(tool) : ''
}

onBeforeUnmount(() => {
  stopTyping()
})
</script>

<style scoped>
.code-assistant-body {
  flex: 1;
  overflow-y: auto;
  padding: 0.95rem;
  scrollbar-color: rgba(125, 211, 252, 0.62) transparent;
  scrollbar-width: thin;
}

.assistant-message,
.user-message {
  display: flex;
  gap: 0.55rem;
  margin-bottom: 0.85rem;
}

.assistant-avatar {
  display: grid;
  width: 1.75rem;
  height: 1.75rem;
  flex: 0 0 auto;
  place-items: center;
  border: 1px solid rgba(125, 211, 252, 0.38);
  border-radius: 50%;
  background: rgba(14, 165, 233, 0.18);
  color: rgb(224, 242, 254);
  box-shadow: 0 0 18px rgba(14, 165, 233, 0.16);
}

.assistant-bubble,
.user-bubble {
  max-width: 82%;
  border: 1px solid rgba(255, 255, 255, 0.13);
  overflow-wrap: anywhere;
  white-space: pre-wrap;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
}

.assistant-bubble {
  border-radius: 14px 14px 14px 4px;
  background: rgba(255, 255, 255, 0.14);
  padding: 0.65rem 0.75rem;
  color: rgba(248, 250, 252, 0.96);
}

.assistant-text {
  font-size: 0.875rem;
  line-height: 1.6;
}

.assistant-text.muted {
  margin-top: 0.25rem;
  color: rgba(226, 232, 240, 0.72);
}

.user-message {
  justify-content: flex-end;
}

.user-bubble {
  border-radius: 14px 14px 4px 14px;
  background: rgba(14, 116, 205, 0.58);
  padding: 0.62rem 0.72rem;
  color: white;
  font-size: 0.875rem;
  line-height: 1.55;
  box-shadow: 0 10px 24px rgba(14, 116, 205, 0.18);
}

.message-time {
  display: block;
  margin-top: 0.3rem;
  color: rgba(226, 232, 240, 0.55);
  font-size: 0.6875rem;
  text-align: right;
}

.tool-bubble {
  max-width: 90%;
}

.tool-event-message {
  margin-left: 0.2rem;
}

.tool-avatar {
  border-color: rgba(56, 189, 248, 0.42);
  background: rgba(2, 132, 199, 0.18);
}

.tool-event-bubble {
  width: min(24rem, 88%);
  max-width: 88%;
  border-radius: 12px;
  background: rgba(15, 23, 42, 0.55);
  padding: 0.62rem 0.7rem;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
}

.tool-event-bubble.tool-running {
  border-color: rgba(125, 211, 252, 0.32);
}

.tool-event-bubble.tool-success {
  border-color: rgba(74, 222, 128, 0.26);
}

.tool-event-bubble.tool-error {
  border-color: rgba(248, 113, 113, 0.36);
}

.tool-event-title {
  color: rgba(248, 250, 252, 0.94);
  font-size: 0.8125rem;
  font-weight: 800;
  line-height: 1.45;
}

.tool-event-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 0.38rem;
  margin-top: 0.28rem;
  color: rgba(186, 230, 253, 0.62);
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', monospace;
  font-size: 0.66rem;
}

.tool-event-meta span:not(:last-child)::after {
  content: '·';
  margin-left: 0.38rem;
  color: rgba(148, 163, 184, 0.5);
}

.tool-event-detail {
  margin-top: 0.38rem;
  color: rgba(203, 213, 225, 0.72);
  font-size: 0.72rem;
  line-height: 1.45;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

.tool-status {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  color: rgb(186, 230, 253);
  font-size: 0.8125rem;
  font-weight: 700;
}

.tool-preview {
  margin-top: 0.35rem;
  color: rgba(226, 232, 240, 0.62);
  font-size: 0.75rem;
  line-height: 1.45;
}

.code-assistant-body::-webkit-scrollbar {
  width: 4px;
}
.code-assistant-body::-webkit-scrollbar-thumb {
  background: rgba(125, 211, 252, 0.4);
  border-radius: 2px;
}
</style>
