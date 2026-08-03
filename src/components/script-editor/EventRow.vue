<template>
  <div class="relative">
    <span
      class="absolute -left-[21px] top-3 w-[9px] h-[9px] rounded-full border-2 border-[#2b3a4a]"
      :style="{ background: spec?.color ?? '#64748b' }"
    ></span>
    <span
      v-if="conditionText"
      class="absolute -left-[21px] -top-[7px] border border-[rgba(251,191,36,0.32)] rounded-[3px] px-[5px] font-mono text-[9px] whitespace-nowrap text-[#fcd34d] bg-[rgba(251,191,36,0.16)]"
      >若 {{ conditionText }}</span
    >
    <span
      v-if="varBadge"
      class="absolute -left-[21px] bottom-[2px] border border-[rgba(34,211,238,0.35)] rounded-[3px] px-[5px] font-mono text-[9px] whitespace-nowrap text-[#67e8f9] bg-[rgba(34,211,238,0.14)]"
      >{{ varBadge }}</span
    >

    <div
      class="group flex items-start gap-2 rounded-lg border border-transparent px-[9px] py-1.5 transition-all hover:bg-white/[0.07]"
      :class="{
        '!border-[rgba(121,217,255,0.4)] !bg-[rgba(121,217,255,0.12)]':
          index === store.selectedEvent,
      }"
      @click="store.selectedEvent = index"
    >
      <span
        class="shrink-0 rounded-[5px] border px-[7px] py-0.5 text-[0.7rem] font-medium leading-[1.5] whitespace-nowrap"
        :style="{
          color: spec?.color,
          borderColor: (spec?.color ?? '#64748b') + '55',
          background: (spec?.color ?? '#64748b') + '14',
        }"
      >
        {{ spec?.label ?? eventType }}
      </span>

      <span class="min-w-0 flex-1 overflow-hidden truncate text-[0.78rem] leading-[1.7] text-white/[0.72]">
        <template
          v-for="(part, i) in highlighted"
          :key="i"
        >
          <span
            v-if="part.token"
            class="text-[var(--accent-color)] opacity-80"
            >{{ part.text }}</span
          >
          <template v-else>{{ part.text }}</template>
        </template>
      </span>

      <span
        v-if="errorCount"
        class="shrink-0 rounded px-[5px] py-px text-[0.62rem] leading-[1.6] whitespace-nowrap border border-[rgba(248,113,113,0.35)] text-[#fca5a5] bg-[rgba(248,113,113,0.15)]"
        >{{ errorCount }} 个错误</span
      >
      <span
        v-else-if="warnCount"
        class="shrink-0 rounded px-[5px] py-px text-[0.62rem] leading-[1.6] whitespace-nowrap border border-[rgba(251,191,36,0.3)] text-[#fcd34d] bg-[rgba(251,191,36,0.15)]"
        >{{ warnCount }} 个提醒</span
      >
      <span
        v-if="event.duration !== undefined"
        class="shrink-0 rounded px-[5px] py-px text-[0.62rem] leading-[1.6] whitespace-nowrap border border-[rgba(251,191,36,0.3)] text-[#fcd34d] bg-[rgba(251,191,36,0.15)]"
        >duration 无效</span
      >

      <button
        class="shrink-0 rounded px-[3px] text-[11px] leading-[1.7] text-white/25 opacity-0 transition-all group-hover:opacity-100 hover:text-[var(--accent-color)] hover:bg-white/[0.1]"
        title="复制"
        @click.stop="store.duplicateEvent(index)"
      >
        ⧉
      </button>
      <button
        v-if="canMoveUp"
        class="shrink-0 rounded px-[3px] text-[11px] leading-[1.7] text-white/25 opacity-0 transition-all group-hover:opacity-100 hover:text-white/60 hover:bg-white/[0.1]"
        title="上移"
        @click.stop="store.moveEvent(index, index - 1)"
      >▲</button>
      <button
        v-if="canMoveDown"
        class="shrink-0 rounded px-[3px] text-[11px] leading-[1.7] text-white/25 opacity-0 transition-all group-hover:opacity-100 hover:text-white/60 hover:bg-white/[0.1]"
        title="下移"
        @click.stop="store.moveEvent(index, index + 1)"
      >▼</button>
      <button
        class="shrink-0 rounded px-[3px] text-[11px] leading-[1.7] text-white/25 opacity-0 transition-all group-hover:opacity-100 hover:text-[#fca5a5] hover:bg-[rgba(248,113,113,0.15)]"
        title="删除"
        @click.stop="store.removeEvent(index)"
      >
        ✕
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import { eventSummary } from '@/composables/useEventFolding'
import type { ScriptEventData } from '@/api/services/script-editor'

const props = defineProps<{
  index: number
  event: ScriptEventData
}>()

const store = useScriptEditorStore()

const eventType = computed(() =>
  typeof props.event.type === 'string' ? (props.event.type as string) : '',
)

const spec = computed(() => store.eventSpecs[eventType.value])

const conditionText = computed(() =>
  typeof props.event.condition === 'string' && props.event.condition.trim() !== ''
    ? (props.event.condition as string)
    : '',
)

/**
 * 变量相关角标：把「写了变量」的事件一眼标出来，方便在长章节里快速定位。
 * - set_variable：赋值组里所有被写过的变量名（去重）
 * - 其它事件：子结构（choices 选项 / 章节分支 / 赋值组）里的条件或赋值变量
 * 只取变量名本身，不拼整条表达式，避免角标过长。
 */
const varBadge = computed(() => {
  const ev = props.event
  const t = eventType.value

  const condVars = (cond: unknown): string[] => {
    if (typeof cond !== 'string') return []
    const s = cond.trim()
    if (!s) return []
    const varName = s.split(/\s*[!=]+\s*/)[0].trim()
    return varName ? [varName] : []
  }

  const collect = (): string[] => {
    if (t === 'set_variable') {
      const opts = Array.isArray(ev.options) ? (ev.options as Record<string, unknown>[]) : []
      const out: string[] = []
      for (const o of opts) {
        out.push(...condVars(o.condition))
        for (const a of Array.isArray(o.actions) ? (o.actions as Record<string, unknown>[]) : []) {
          const c = a.content
          if (typeof c === 'string') {
            const m = /^\s*(\S+)\s*(?:=|\+=|-=)/.exec(c)
            if (m) out.push(m[1])
          }
        }
      }
      return out
    }
    // choices 选项里的条件；分支/赋值组的条件在摘要里已有，这里补上顶层没有的
    if (t === 'choices') {
      const opts = Array.isArray(ev.options) ? (ev.options as Record<string, unknown>[]) : []
      return opts.flatMap((o) => condVars(o.condition))
    }
    return []
  }

  const vars = [...new Set(collect())]
  return vars.length ? `⚙ ${vars.join(', ')}` : ''
})

const diagnostics = computed(() => store.chapterDiagnostics[props.index] ?? [])
const errorCount = computed(() => diagnostics.value.filter((d) => d.severity === 'error').length)
const warnCount = computed(() => diagnostics.value.filter((d) => d.severity === 'warn').length)

/** 把摘要按 %player% 切开，占位符用强调色标出来 */
const highlighted = computed(() => {
  const text = eventSummary(props.event)
  const parts: { text: string; token: boolean }[] = []
  let rest = text
  while (true) {
    const at = rest.indexOf('%player%')
    if (at === -1) break
    if (at > 0) parts.push({ text: rest.slice(0, at), token: false })
    parts.push({ text: '%player%', token: true })
    rest = rest.slice(at + 8)
  }
  if (rest) parts.push({ text: rest, token: false })
  return parts
})

const isChapterEnd = computed(() => props.event.type === 'chapter_end')
const lastMovableIdx = computed(() => {
  const total = store.chapter?.events.length ?? 0
  return total > 0 && store.chapter?.events[total - 1]?.type === 'chapter_end' ? total - 2 : total - 1
})
const canMoveUp = computed(() => !isChapterEnd.value && props.index > 0)
const canMoveDown = computed(() => !isChapterEnd.value && props.index < lastMovableIdx.value)
</script>
