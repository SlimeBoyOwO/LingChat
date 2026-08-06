<template>
  <div
    class="rounded-[10px]
      border
      px-[13px]
      py-[11px]"
    :class="[tone.border, tone.bg]"
  >
    <div class="flex
      items-center
      gap-2">
      <span class="text-[1rem]
        leading-none">{{ tone.emoji }}</span>
      <span class="font-mono
        text-[0.82rem]
        text-white/90">{{ run.tool }}</span>
      <span
        class="ml-auto
          inline-flex
          items-center
          rounded-full
          border
          px-[7px]
          py-[1px]
          text-[0.68rem]"
        :class="statusMap[run.status].cls"
      >
        {{ statusMap[run.status].text }}
      </span>
    </div>

    <!-- 等待审批：内联允许/拒绝 -->
    <div
      v-if="run.status === 'pending'"
      class="mt-2.5
        flex
        items-center
        gap-2"
    >
      <button
        class="inline-flex
          items-center
          gap-1
          rounded-lg
          border
          border-emerald-400/40
          bg-emerald-400/15
          px-3
          py-1
          text-[0.78rem]
          text-emerald-300
          transition-all
          duration-200
          hover:bg-emerald-400/25"
        @click="$emit('allow')"
      >
        {{ t('scriptEditor.agentTool.allow') }}
      </button>
      <button
        class="inline-flex
          items-center
          gap-1
          rounded-lg
          border
          border-red-400/35
          bg-red-400/12
          px-3
          py-1
          text-[0.78rem]
          text-red-300
          transition-all
          duration-200
          hover:bg-red-400/25"
        @click="$emit('deny')"
      >
        {{ t('scriptEditor.agentTool.deny') }}
      </button>
      <span class="text-[0.7rem]
        text-white/40">{{
        t('scriptEditor.agentTool.approvalHint')
      }}</span>
    </div>

    <!-- 参数 / 结果，点击展开 -->
    <button
      v-if="(run.args && Object.keys(run.args).length > 0) || run.output"
      class="mt-2
        inline-flex
        items-center
        gap-1
        text-[0.72rem]
        text-white/45
        transition-colors
        hover:text-white/80"
      @click="showDetails = !showDetails"
    >
      {{
        showDetails
          ? t('scriptEditor.agentTool.hideDetails')
          : t('scriptEditor.agentTool.showDetails')
      }}
    </button>
    <div
      v-if="showDetails"
      class="mt-2
        space-y-2
        overflow-hidden"
    >
      <div
        v-if="hasArgs"
        class="rounded-lg
          border
          border-white/10
          bg-black/25
          px-2.5
          py-2"
      >
        <div class="mb-1
          text-[0.68rem]
          text-white/40">{{ t('scriptEditor.agentTool.args') }}</div>
        <pre
          class="max-h-40
            overflow-y-auto
            whitespace-pre-wrap
            font-mono
            text-[0.72rem]
            text-white/75"
          >{{ argsText }}</pre
        >
      </div>
      <div
        v-if="run.output"
        class="rounded-lg
          border
          border-white/10
          bg-black/25
          px-2.5
          py-2"
      >
        <div class="mb-1
          text-[0.68rem]
          text-white/40">
          {{ t('scriptEditor.agentTool.result') }}
        </div>
        <pre
          class="max-h-52
            overflow-y-auto
            whitespace-pre-wrap
            font-mono
            text-[0.72rem]
            text-white/75"
          >{{ run.output }}</pre
        >
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import type { ToolRun, ToolStatus } from '@/stores/modules/agent/state'

const { t } = useI18n()
const props = defineProps<{ run: ToolRun }>()
defineEmits<{ (e: 'allow'): void; (e: 'deny'): void }>()

const showDetails = ref(false)

interface Tone {
  border: string
  bg: string
  emoji: string
}

const TONES: Record<string, Tone> = {
  indigo: { border: 'border-indigo-400/35', bg: 'bg-indigo-400/8', emoji: '📖' },
  amber: { border: 'border-amber-400/35', bg: 'bg-amber-400/8', emoji: '💻' },
  emerald: { border: 'border-emerald-400/35', bg: 'bg-emerald-400/8', emoji: '📄' },
  red: { border: 'border-red-400/35', bg: 'bg-red-400/8', emoji: '🗑️' },
}

const toneOf = (tool: string): string => {
  if (tool === 'execute_command') return 'amber'
  if (tool === 'read_skill' || tool === 'list_skills') return 'indigo'
  if (tool === 'delete_file') return 'red'
  return 'emerald'
}

const tone = computed<Tone>(() => TONES[toneOf(props.run.tool)] ?? TONES.emerald)

const statusMap: Record<ToolStatus, { text: string; cls: string }> = {
  running: {
    text: t('scriptEditor.agentTool.statusRunning'),
    cls: 'text-amber-300 border-amber-300/30 bg-amber-300/10',
  },
  pending: {
    text: t('scriptEditor.agentTool.statusApproval'),
    cls: 'text-blue-300 border-blue-300/30 bg-blue-300/10',
  },
  done: {
    text: t('scriptEditor.agentTool.statusDone'),
    cls: 'text-emerald-300 border-emerald-300/30 bg-emerald-300/10',
  },
  error: {
    text: t('scriptEditor.agentTool.statusFailed'),
    cls: 'text-red-300 border-red-300/30 bg-red-300/10',
  },
  denied: {
    text: t('scriptEditor.agentTool.statusDenied'),
    cls: 'text-red-300 border-red-300/30 bg-red-300/10',
  },
}

const hasArgs = computed(() => props.run.args && Object.keys(props.run.args).length > 0)

const argsText = computed(() => {
  try {
    return JSON.stringify(props.run.args, null, 2)
  } catch {
    return String(props.run.args)
  }
})
</script>
