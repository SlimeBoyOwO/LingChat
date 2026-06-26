<template>
  <section v-if="view === 'tool_calls'" class="space-y-4">
    <div class="flex items-center justify-between">
      <div>
        <h3 class="text-xl font-bold text-brand">工具调用记录</h3>
        <p class="text-xs text-white/70 mt-1">最近 50 条 AI 内部工具调用会显示在这里</p>
      </div>
      <button
        class="px-3 py-2 rounded-lg bg-white/10 hover:bg-white/20 text-white text-sm flex items-center gap-2"
        @click="clearLogs"
      >
        <Trash2 :size="16" />
        清空
      </button>
    </div>

    <div
      v-if="toolCallLogs.length === 0"
      class="min-h-56 border border-dashed border-cyan-400/40 rounded-lg flex flex-col items-center justify-center text-white/60"
    >
      <Wrench :size="34" class="mb-3" />
      <p>还没有工具调用记录</p>
    </div>

    <div v-else class="space-y-3">
      <article
        v-for="entry in toolCallLogs"
        :key="entry.id || `${entry.tool}-${entry.timestamp}`"
        class="rounded-lg border border-cyan-300/25 bg-slate-900/35 p-4 text-white"
      >
        <div class="flex items-center justify-between gap-3">
          <div class="flex items-center gap-2 min-w-0">
            <CheckCircle2
              v-if="entry.status === 'success'"
              :size="18"
              class="text-emerald-300 shrink-0"
            />
            <LoaderCircle
              v-else-if="entry.status === 'running'"
              :size="18"
              class="text-cyan-300 animate-spin shrink-0"
            />
            <XCircle v-else :size="18" class="text-rose-300 shrink-0" />
            <strong class="truncate">{{ entry.tool }}</strong>
          </div>
          <span class="px-2 py-0.5 rounded-full text-xs" :class="statusClass(entry.status)">
            {{ statusText(entry.status) }}
          </span>
        </div>

        <p class="text-xs text-white/60 mt-2">{{ formatTime(entry.timestamp) }}</p>

        <div class="mt-3 space-y-2">
          <div class="rounded-lg bg-black/20 p-3">
            <div class="text-xs text-white/60 mb-1">参数</div>
            <pre class="text-xs whitespace-pre-wrap break-words">{{
              formatJson(entry.arguments || {})
            }}</pre>
          </div>
          <div v-if="entry.preview" class="rounded-lg bg-black/20 p-3">
            <div class="text-xs text-white/60 mb-1">结果预览</div>
            <pre class="text-xs whitespace-pre-wrap break-words">{{ entry.preview }}</pre>
          </div>
        </div>
      </article>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { CheckCircle2, LoaderCircle, Trash2, Wrench, XCircle } from 'lucide-vue-next'
import { useUIStore, type ToolCallLog } from '@/stores/modules/ui/ui'

const props = defineProps<{
  view: string
}>()

const uiStore = useUIStore()

const toolCallLogs = computed(() => uiStore.toolCallLogs)

const clearLogs = () => {
  uiStore.clearToolCallLogs()
}

const formatJson = (value: unknown) => JSON.stringify(value, null, 2)

const formatTime = (value: string) => {
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString()
}

const statusText = (status: string) => {
  if (status === 'success') return '完成'
  if (status === 'running') return '运行中'
  if (status === 'error') return '错误'
  return status
}

const statusClass = (status: string) => {
  if (status === 'success') return 'bg-emerald-400/20 text-emerald-100'
  if (status === 'running') return 'bg-cyan-400/20 text-cyan-100'
  return 'bg-rose-400/20 text-rose-100'
}
</script>
