import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'
import { i18n } from '@/locales'

/** 网页搜索工具配置（与后端 WebSearchSettings 对应，字段保持 snake_case）。 */
export interface WebSearchSettings {
  enabled: boolean
  /** true = 模型 API 内置联网（免 Key）；false = 独立搜索端点 + api_key */
  use_builtin: boolean
  /** 独立端点模式的搜索提供商："kimi" | "bocha" | "custom"（仅 custom 用 base_url） */
  provider: string
  api_key: string
  base_url: string
  proxy_enabled: boolean
  proxy_addr: string
  max_results: number
  /** true = 搜索结果不带来源/网址，模型回答中不显示原始搜索结果 */
  hide_search_results: boolean
}

export interface ToolSettings {
  web_search: WebSearchSettings
  /** 分组开关：组名 → 是否启用（schedule/memory/character/scene/status/clock/skills/file_ops/command） */
  groups: Record<string, boolean>
  /** 命令执行：免确认直接运行 shell（危险，默认关闭） */
  command_auto_approve: boolean
  /** 命令执行：检测到删除操作时免确认继续执行（危险，默认关闭） */
  command_delete_auto_approve: boolean
  /** 删除文件：免确认直接删除（危险，默认关闭） */
  file_delete_auto_approve: boolean
  /** 文件操作：允许访问沙箱（data/）之外的路径（默认关闭） */
  file_ops_allow_any_path: boolean
}

/** 「其他工具」分组（与后端 TOOL_GROUPS 对齐；web_search 有独立设置区） */
export const TOOL_GROUP_KEYS = [
  'schedule',
  'memory',
  'character',
  'scene',
  'status',
  'clock',
  'skills',
  'file_ops',
  'command',
] as const

/** 工具的界面友好名（通知与调用记录用），查不到翻译时回退原始名。 */
export function toolDisplayName(tool: string): string {
  const key = `ui.toolCalls.tools.${tool}`
  return i18n.global.te(key) ? i18n.global.t(key) : tool
}

/** 后端 `ai:tool_call` 事件的载荷 + 前端补充的时间戳。 */
export interface ToolCallRecord {
  call_id?: string
  tool: string
  ok: boolean
  summary: string
  error: string | null
  /** 调用参数（截断至 1000 字符），用于展开详情 */
  arguments: string
  /** 工具返回结果（截断至 1000 字符），用于展开详情 */
  result: string
  time: string
}

export interface ToolActivityEvent {
  call_id: string
  tool: string
  phase: 'started' | 'finished'
  ok?: boolean | null
  arguments: string
}

export interface ToolActivityState {
  callId: string
  tool: string
  arguments: string
  status: 'running' | 'success' | 'failure'
  sequence: number
}

/** 顶栏正在显示的工具活动；结束状态短暂停留后自动淡出。 */
export const currentToolActivity = ref<ToolActivityState | null>(null)

/** 工具调用参数的流式生成进度（顶栏「正在生成…N 字」实时提示）。 */
export const toolCallPreparing = ref<{ tool: string; chars: number } | null>(null)

/** 接收后端参数生成进度事件（工具块开始与每个参数增量都会触发）。 */
export function handleToolCallProgress(payload: { tool: string; chars: number }) {
  if (!payload.tool?.trim()) return
  toolCallPreparing.value = { tool: payload.tool, chars: payload.chars }
}

/** 清除「正在生成」进度提示（一轮 LLM 流结束 / 工具进入执行阶段 / 异常中断时调用）。 */
export function clearToolCallPreparing() {
  toolCallPreparing.value = null
}

// 后台命令最长运行 60 分钟，再留 5 分钟给事件投递与系统休眠恢复。
const ACTIVE_WATCHDOG_MS = 65 * 60 * 1000
const FINISHED_VISIBLE_MS = 2200
const activeToolCalls = new Map<string, ToolActivityState>()
const watchdogTimers = new Map<string, ReturnType<typeof setTimeout>>()
let sequence = 0
let finishedClearTimer: ReturnType<typeof setTimeout> | null = null

function clearFinishedTimer() {
  if (finishedClearTimer !== null) {
    clearTimeout(finishedClearTimer)
    finishedClearTimer = null
  }
}

function clearWatchdog(callId: string) {
  const timer = watchdogTimers.get(callId)
  if (timer !== undefined) clearTimeout(timer)
  watchdogTimers.delete(callId)
}

function latestActiveTool(): ToolActivityState | null {
  let latest: ToolActivityState | null = null
  for (const activity of activeToolCalls.values()) {
    if (latest === null || activity.sequence > latest.sequence) latest = activity
  }
  return latest
}

function isBackgroundCommand(activity: ToolActivityState): boolean {
  if (activity.tool !== 'execute_command') return false
  try {
    const args = JSON.parse(activity.arguments) as Record<string, unknown>
    return args.run_in_background === true
  } catch {
    return false
  }
}

function showFinished(activity: ToolActivityState, ok: boolean) {
  const next = latestActiveTool()
  if (next) {
    currentToolActivity.value = next
    return
  }

  const finished: ToolActivityState = {
    ...activity,
    status: ok ? 'success' : 'failure',
  }
  currentToolActivity.value = finished
  clearFinishedTimer()
  finishedClearTimer = setTimeout(() => {
    if (
      currentToolActivity.value?.callId === finished.callId &&
      currentToolActivity.value.status !== 'running'
    ) {
      currentToolActivity.value = null
    }
    finishedClearTimer = null
  }, FINISHED_VISIBLE_MS)
}

/** 接收后端生命周期事件，并正确处理连续/重叠的工具调用。 */
export function handleToolActivity(event: ToolActivityEvent) {
  if (!event.call_id?.trim() || !event.tool?.trim()) return

  if (event.phase === 'started') {
    // 参数已合并完整、进入执行阶段：清掉「正在生成」进度提示
    clearToolCallPreparing()
    clearFinishedTimer()
    clearWatchdog(event.call_id)
    const activity: ToolActivityState = {
      callId: event.call_id,
      tool: event.tool,
      arguments: event.arguments || '{}',
      status: 'running',
      sequence: ++sequence,
    }
    activeToolCalls.set(event.call_id, activity)
    currentToolActivity.value = activity
    watchdogTimers.set(
      event.call_id,
      setTimeout(() => {
        const stale = activeToolCalls.get(event.call_id)
        if (!stale || stale.sequence !== activity.sequence) return
        activeToolCalls.delete(event.call_id)
        watchdogTimers.delete(event.call_id)
        showFinished(stale, false)
      }, ACTIVE_WATCHDOG_MS),
    )
    return
  }

  const activity = activeToolCalls.get(event.call_id) ?? {
    callId: event.call_id,
    tool: event.tool,
    arguments: event.arguments || '{}',
    status: 'running' as const,
    sequence: ++sequence,
  }
  clearWatchdog(event.call_id)
  activeToolCalls.delete(event.call_id)
  showFinished(activity, event.ok !== false)
}

/** AI 请求异常结束时清理前台调用；已脱离当前生成的后台命令继续保留。 */
export function interruptToolActivities() {
  clearToolCallPreparing()
  const visible = currentToolActivity.value
  for (const [callId, activity] of [...activeToolCalls.entries()]) {
    if (isBackgroundCommand(activity)) continue
    clearWatchdog(callId)
    activeToolCalls.delete(callId)
  }
  const background = latestActiveTool()
  if (background) {
    currentToolActivity.value = background
  } else if (visible?.status === 'running' && !isBackgroundCommand(visible)) {
    showFinished(visible, false)
  }
}

const MAX_HISTORY = 50

/** 最近的工具调用记录（内存态，最新在前），供「工具调用」页面展示。 */
export const recentToolCalls = ref<ToolCallRecord[]>([])

export function pushToolCallRecord(record: ToolCallRecord) {
  recentToolCalls.value.unshift(record)
  if (recentToolCalls.value.length > MAX_HISTORY) {
    recentToolCalls.value.length = MAX_HISTORY
  }
}

/** 清空工具调用记录。 */
export function clearToolCallRecords() {
  recentToolCalls.value = []
}

export function getToolSettings(): Promise<ToolSettings> {
  return invoke<ToolSettings>('get_tool_settings')
}

export function saveToolSettings(settings: ToolSettings): Promise<void> {
  return invoke<void>('save_tool_settings', { settings })
}

/** 直接执行一次网页搜索；失败时 Promise reject 携带后端错误信息。 */
export function testWebSearch(query: string): Promise<string> {
  return invoke<string>('test_web_search', { query })
}
