<template>
  <Transition
    enter-active-class="transition-all duration-200 ease-out"
    leave-active-class="transition-all duration-200 ease-in"
    enter-from-class="opacity-0 -translate-x-1"
    leave-to-class="opacity-0 -translate-x-1"
    mode="out-in"
  >
    <div
      v-if="(activity || preparing) && !uiStore.showSettings"
      :key="viewKey"
      class="hidden
        pointer-events-none
        mr-2
        h-10
        min-w-0
        max-w-[min(36vw,28rem)]
        items-center
        xl:flex"
      role="status"
      aria-live="polite"
      :title="statusText"
    >
      <div
        class="flex
          min-w-0
          items-center
          gap-1.5
          text-xs
          font-medium
          tracking-wide
          drop-shadow-sm"
        :class="statusClass"
      >
        <LoaderCircle
          v-if="preparing || activity?.status === 'running'"
          :size="14"
          class="shrink-0
            animate-spin"
        />
        <CheckCircle2
          v-else-if="activity?.status === 'success'"
          :size="14"
          class="shrink-0"
        />
        <CircleAlert
          v-else
          :size="14"
          class="shrink-0"
        />
        <span class="truncate">{{ statusText }}</span>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { CheckCircle2, CircleAlert, LoaderCircle } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { currentToolActivity, toolCallPreparing } from '@/api/services/tool-settings'
import { useUIStore } from '@/stores/modules/ui/ui'

const uiStore = useUIStore()
const { t, te } = useI18n()
const activity = currentToolActivity
// 工具调用参数的流式生成进度；存在时优先于执行状态展示
const preparing = toolCallPreparing

const viewKey = computed(() => {
  if (preparing.value) return `preparing-${preparing.value.tool}`
  const current = activity.value
  return current ? `${current.callId}-${current.status}` : 'idle'
})

const readTools = new Set([
  'list_skills',
  'read_skill',
  'list_files',
  'read_file',
  'memory_get_current',
  'memory_get_notes',
  'schedule_get_all',
  'character_list',
  'scene_list',
  'status_get_current',
  'status_get_scene',
  'get_current_time',
])
const updateTools = new Set([
  'schedule_add_todo',
  'schedule_update_todo',
  'schedule_delete_todo',
  'memory_add_note',
  'memory_update_note',
  'memory_delete_note',
])
const switchTools = new Set(['character_switch', 'scene_switch'])

const toolLabel = computed(() => {
  const tool = activity.value?.tool ?? ''
  const key = `ui.toolCalls.tools.${tool}`
  return tool && te(key) ? t(key) : tool
})

function compact(value: string): string {
  const normalized = value.replace(/\s+/g, ' ').trim()
  if (normalized.length <= 44) return normalized
  return `${normalized.slice(0, 20)}…${normalized.slice(-20)}`
}

const target = computed(() => {
  const current = activity.value
  if (!current) return ''
  try {
    const args = JSON.parse(current.arguments) as Record<string, unknown>
    const keys = ['search_files', 'grep_files', 'web_search'].includes(current.tool)
      ? ['query', 'pattern', 'path']
      : current.tool === 'execute_command'
        ? ['description']
        : ['path', 'name', 'text', 'query']
    for (const key of keys) {
      const value = args[key]
      if (typeof value === 'string' && value.trim()) return compact(value)
    }
  } catch {
    // 参数仅用于展示，解析失败时退回工具名称。
  }
  return toolLabel.value
})

function isBackgroundCommand(current: NonNullable<typeof activity.value>): boolean {
  if (current.tool !== 'execute_command') return false
  try {
    const args = JSON.parse(current.arguments) as Record<string, unknown>
    return args.run_in_background === true
  } catch {
    return false
  }
}

function runningKey(current: NonNullable<typeof activity.value>): string {
  const tool = current.tool
  if (tool === 'write_file') return 'writing'
  if (tool === 'edit_file') return 'editing'
  if (tool === 'delete_file') return 'deleting'
  if (['web_search', 'search_files', 'grep_files'].includes(tool)) return 'searching'
  if (tool === 'execute_command') {
    if (isBackgroundCommand(current)) return 'backgroundExecuting'
    return 'executing'
  }
  if (updateTools.has(tool)) return 'updating'
  if (switchTools.has(tool)) return 'switching'
  if (readTools.has(tool)) return 'reading'
  return 'calling'
}

const statusText = computed(() => {
  const pending = preparing.value
  if (pending) {
    const key = `ui.toolCalls.tools.${pending.tool}`
    const label = te(key) ? t(key) : pending.tool
    return t('ui.toolActivity.preparing', { tool: label, chars: pending.chars })
  }
  const current = activity.value
  if (!current) return ''
  if (current.status === 'success') {
    return t('ui.toolActivity.completed', { tool: toolLabel.value })
  }
  if (current.status === 'failure') {
    return t('ui.toolActivity.failed', { tool: toolLabel.value })
  }
  return t(`ui.toolActivity.${runningKey(current)}`, {
    target: target.value,
    tool: toolLabel.value,
  })
})

const statusClass = computed(() => {
  if (preparing.value) return 'text-sky-200/90'
  switch (activity.value?.status) {
    case 'success':
      return 'text-emerald-300/90'
    case 'failure':
      return 'text-amber-300/90'
    default:
      return 'text-sky-200/90'
  }
})
</script>
