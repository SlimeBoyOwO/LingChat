<template>
  <!-- 视图：工具调用（设置已移至「高级设置 → 工具配置」，这里保留入口与调用提示） -->
  <div
    v-if="uiStore.scheduleView === 'tool_calls'"
    class="grid grid-cols-1 sm:grid-cols-1 lg:grid-cols-1 p-1"
  >
    <!-- 跳转高级设置 -->
    <div class="mb-6">
      <p class="text-sm text-gray-300 mb-3">{{ $t('ui.toolCalls.settingsMovedHint') }}</p>
      <div
        class="inline-flex items-center gap-2 px-5 py-2.5 bg-brand text-white border-none rounded-lg cursor-pointer text-sm font-medium transition-colors duration-200 hover:bg-[#0056b3]"
        @click="goToToolSettings"
      >
        <Wrench :size="16" />
        {{ $t('ui.toolCalls.goToSettings') }}
      </div>
    </div>

    <!-- 调用提示（最近记录，最多 50 条） -->
    <div>
      <div class="flex items-center justify-between mb-2">
        <h3 class="text-brand font-bold">{{ $t('ui.toolCalls.historyTitle') }}</h3>
        <button
          v-if="recentToolCalls.length > 0"
          class="flex items-center gap-1.5 px-3 py-1.5 text-xs text-white/70 border border-white/20 rounded-lg cursor-pointer transition-colors duration-200 hover:bg-white/10 hover:text-white"
          @click="clearToolCallRecords"
        >
          <Trash2 :size="14" />
          {{ $t('ui.toolCalls.clearHistory') }}
        </button>
      </div>
      <p v-if="recentToolCalls.length === 0" class="text-sm text-gray-400">
        {{ $t('ui.toolCalls.historyEmpty') }}
      </p>
      <ul v-else class="space-y-2">
        <li
          v-for="(record, index) in recentToolCalls"
          :key="index"
          class="text-sm bg-white/5 rounded-lg px-3 py-2 cursor-pointer transition-colors duration-200 hover:bg-white/10"
          @click="toggleExpand(index)"
        >
          <div class="flex items-center gap-3">
            <CheckCircle2 v-if="record.ok" :size="16" class="text-green-400 shrink-0" />
            <XCircle v-else :size="16" class="text-red-400 shrink-0" />
            <span class="text-gray-400 shrink-0">{{ record.time }}</span>
            <span class="text-brand shrink-0">{{ toolLabel(record.tool) }}</span>
            <span class="text-white truncate">{{
              record.ok ? displaySummary(record) : record.error || displaySummary(record)
            }}</span>
            <ChevronDown
              :size="14"
              class="ml-auto shrink-0 text-gray-400 transition-transform duration-200"
              :class="{ 'rotate-180': expandedIndex === index }"
            />
          </div>
          <!-- 展开详情：调用参数与返回结果 -->
          <div v-if="expandedIndex === index" class="mt-2 space-y-2 border-t border-white/10 pt-2">
            <div>
              <p class="text-xs text-gray-400 mb-1">{{ $t('ui.toolCalls.detailArgs') }}</p>
              <pre
                class="text-xs text-white/80 bg-black/30 rounded p-2 whitespace-pre-wrap break-all max-h-40 overflow-y-auto"
                >{{ prettyJson(record.arguments) }}</pre
              >
            </div>
            <div>
              <p class="text-xs text-gray-400 mb-1">{{ $t('ui.toolCalls.detailResult') }}</p>
              <pre
                class="text-xs text-white/80 bg-black/30 rounded p-2 whitespace-pre-wrap break-all max-h-60 overflow-y-auto"
                >{{ prettyJson(record.result) }}</pre
              >
            </div>
          </div>
        </li>
      </ul>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'
import {
  recentToolCalls,
  clearToolCallRecords,
  toolDisplayName,
  type ToolCallRecord,
} from '@/api/services/tool-settings'
import { CheckCircle2, XCircle, Wrench, Trash2, ChevronDown } from 'lucide-vue-next'
import { i18n } from '@/locales'

const uiStore = useUIStore()

const toolLabel = toolDisplayName

// 展开详情：同一时间只展开一条记录
const expandedIndex = ref<number | null>(null)

const toggleExpand = (index: number) => {
  expandedIndex.value = expandedIndex.value === index ? null : index
}

// 无参工具的摘要会退化成 "{}"，显示为友好文案
const displaySummary = (record: ToolCallRecord) => {
  const summary = record.summary.trim()
  return summary === '{}' || summary === '' ? i18n.global.t('ui.toolCalls.noArgs') : record.summary
}

// 尝试把 JSON 字符串格式化展示，失败则原样返回
const prettyJson = (raw: string) => {
  if (!raw) return '—'
  try {
    return JSON.stringify(JSON.parse(raw), null, 2)
  } catch {
    return raw
  }
}

// 跳转到「高级设置 → 工具配置」子标签（打开设置面板会自动遮住日程弹窗）
const goToToolSettings = () => {
  uiStore.advanceTab = 'tools'
  uiStore.setSettingsTab('advance')
  uiStore.toggleSettings(true)
}
</script>
