<template>
  <div class="relative">
    <span
      class="absolute
        -left-[21px]
        top-3
        w-[9px]
        h-[9px]
        rounded-full
        border-2
        border-[#2b3a4a]"
      :style="{ background: spec?.color ?? '#64748b' }"
    ></span>
    <span
      v-if="conditionText"
      class="absolute
        -left-[21px]
        -top-[7px]
        border
        border-[rgba(251,191,36,0.32)]
        rounded-[3px]
        px-[5px]
        font-mono
        text-[9px]
        whitespace-nowrap
        text-[#fcd34d]
        bg-[rgba(251,191,36,0.16)]"
      >{{ t('scriptEditor.eventRow.conditionPrefix', { condition: conditionText }) }}</span
    >

    <div
      class="group
        flex
        items-start
        gap-2
        rounded-lg
        border
        border-transparent
        px-[9px]
        py-1.5
        transition-all
        hover:bg-white/[0.07]"
      :class="{
        [`!border-[rgba(121,217,255,0.4)]
        !bg-[rgba(121,217,255,0.12)]`]: index === store.selectedEvent,
      }"
      @click="store.selectedEvent = index"
    >
      <span
        class="shrink-0
          rounded-[5px]
          border
          px-[7px]
          py-0.5
          text-[0.7rem]
          font-medium
          leading-[1.5]
          whitespace-nowrap"
        :style="{
          color: spec?.color,
          borderColor: (spec?.color ?? '#64748b') + '55',
          background: (spec?.color ?? '#64748b') + '14',
        }"
      >
        {{ spec ? eventLabelOf(spec) : eventType }}
      </span>

      <span
        class="min-w-0
          flex-1
          overflow-hidden
          truncate
          text-[0.78rem]
          leading-[1.7]
          text-white/[0.72]"
      >
        <template
          v-for="(part, i) in highlighted"
          :key="i"
        >
          <span
            v-if="part.token"
            class="text-[var(--accent-color)]
              opacity-80"
            >{{ part.text }}</span
          >
          <template v-else>{{ part.text }}</template>
        </template>
      </span>

      <!-- 变量摘要：行内右侧角标。变量多时折叠成「N 个变量」，点击展开/收起完整列表 -->
      <span
        v-if="varBadge"
        class="shrink-0
          max-w-[10rem]
          truncate
          cursor-pointer
          rounded
          px-[5px]
          py-px
          text-[0.62rem]
          leading-[1.6]
          whitespace-nowrap
          border
          border-[rgba(34,211,238,0.35)]
          text-[#67e8f9]
          bg-[rgba(34,211,238,0.14)]
          transition-all
          hover:bg-[rgba(34,211,238,0.25)]"
        :title="varBadge"
        @click="varExpanded = !varExpanded"
        >{{ varBadgeLabel }}</span
      >

      <span
        v-if="errorCount"
        class="shrink-0
          rounded
          px-[5px]
          py-px
          text-[0.62rem]
          leading-[1.6]
          whitespace-nowrap
          border
          border-[rgba(248,113,113,0.35)]
          text-[#fca5a5]
          bg-[rgba(248,113,113,0.15)]"
        >{{ t('scriptEditor.chapterFlow.errors', { count: errorCount }) }}</span
      >
      <span
        v-else-if="warnCount"
        class="shrink-0
          rounded
          px-[5px]
          py-px
          text-[0.62rem]
          leading-[1.6]
          whitespace-nowrap
          border
          border-[rgba(251,191,36,0.3)]
          text-[#fcd34d]
          bg-[rgba(251,191,36,0.15)]"
        >{{ t('scriptEditor.chapterFlow.warns', { count: warnCount }) }}</span
      >

      <button
        class="shrink-0
          rounded
          px-[3px]
          text-[11px]
          leading-[1.7]
          text-white/25
          opacity-0
          transition-all
          group-hover:opacity-100
          hover:text-[var(--accent-color)]
          hover:bg-white/[0.1]"
        :title="t('scriptEditor.eventRow.copy')"
        @click.stop="store.duplicateEvent(index)"
      >
        ⧉
      </button>
      <button
        v-if="canMoveUp"
        class="shrink-0
          rounded
          px-[3px]
          text-[11px]
          leading-[1.7]
          text-white/25
          opacity-0
          transition-all
          group-hover:opacity-100
          hover:text-white/60
          hover:bg-white/[0.1]"
        :title="t('scriptEditor.eventRow.moveUp')"
        @click.stop="store.moveEvent(index, index - 1)"
      >
        ▲
      </button>
      <button
        v-if="canMoveDown"
        class="shrink-0
          rounded
          px-[3px]
          text-[11px]
          leading-[1.7]
          text-white/25
          opacity-0
          transition-all
          group-hover:opacity-100
          hover:text-white/60
          hover:bg-white/[0.1]"
        :title="t('scriptEditor.eventRow.moveDown')"
        @click.stop="store.moveEvent(index, index + 1)"
      >
        ▼
      </button>
      <button
        class="shrink-0
          rounded
          px-[3px]
          text-[11px]
          leading-[1.7]
          text-white/25
          opacity-0
          transition-all
          group-hover:opacity-100
          hover:text-[#fca5a5]
          hover:bg-[rgba(248,113,113,0.15)]"
        :title="t('scriptEditor.eventRow.delete')"
        @click.stop="store.removeEvent(index)"
      >
        ✕
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import { eventSummary } from '@/composables/useEventFolding'
import type { ScriptEventData } from '@/api/services/script-editor'
import { eventLabelOf } from '@/locales/schema-i18n'

const { t } = useI18n()
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
const varNames = computed<string[]>(() => [...new Set(collectVars())])

/** 变量名收集：set_variable 取所有被写过的变量；choices 取选项条件里的变量 */
const collectVars = (): string[] => {
  const ev = props.event
  const t = eventType.value

  const condVars = (cond: unknown): string[] => {
    if (typeof cond !== 'string') return []
    const s = cond.trim()
    if (!s) return []
    const varName = s.split(/\s*[!=]+\s*/)[0].trim()
    return varName ? [varName] : []
  }

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

/** 角标完整文本（含 ⚙ 前缀），展开时显示它 */
const varBadge = computed(() => (varNames.value.length ? `⚙ ${varNames.value.join(', ')}` : ''))

/** 变量多时默认折叠成「N 个变量」，点击展开 */
const varExpanded = ref(false)

const varBadgeLabel = computed(() => {
  if (varExpanded.value || varNames.value.length <= 1) return varBadge.value
  return t('scriptEditor.eventRow.vars', { count: varNames.value.length })
})

const diagnostics = computed(() => store.chapterDiagnostics[props.index] ?? [])
const errorCount = computed(() => diagnostics.value.filter((d) => d.severity === 'error').length)
const warnCount = computed(() => diagnostics.value.filter((d) => d.severity === 'warn').length)

/** roleKey → aiName 映射，供摘要显示角色名字（与事件属性下拉一致） */
const roleNameMap = computed<Map<string, string>>(
  () => new Map((store.detail?.characters ?? []).map((c) => [c.roleKey, c.aiName])),
)

/** 把摘要按 %player% 切开，占位符用强调色标出来 */
const highlighted = computed(() => {
  const text = eventSummary(props.event, store.mainRoleDisplayName, roleNameMap.value)
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
  return total > 0 && store.chapter?.events[total - 1]?.type === 'chapter_end'
    ? total - 2
    : total - 1
})
const canMoveUp = computed(() => !isChapterEnd.value && props.index > 0)
const canMoveDown = computed(() => !isChapterEnd.value && props.index < lastMovableIdx.value)
</script>
