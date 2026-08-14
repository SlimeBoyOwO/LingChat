<template>
  <div class="flex
    flex-col
    gap-1">
    <template v-if="!event">
      <p class="py-8
        text-center
        text-sm
        text-white/40">
        {{ t('scriptEditor.eventProperty.emptyHint') }}
      </p>
    </template>

    <template v-else>
      <!-- 事件类型 -->
      <div class="mb-4">
        <label class="inline-flex
          items-center
          font-medium
          text-brand">{{
          t('scriptEditor.eventProperty.eventType')
        }}</label>
        <select
          class="glass-input"
          :value="eventType"
          @change="onTypeChange"
        >
          <optgroup
            v-for="(group, cat) in groupedSpecs"
            :key="cat"
            :label="categoryLabelOf(cat)"
          >
            <option
              v-for="s in group"
              :key="s.typeKey"
              :value="s.typeKey"
            >
              {{ eventLabelOf(s) }}（{{ s.typeKey }}）
            </option>
          </optgroup>
        </select>
        <p class="mt-1
          text-xs
          text-white/40">
          {{
            t('scriptEditor.eventProperty.eventCount', { count: store.schema?.events.length ?? 0 })
          }}
        </p>
      </div>

      <!-- 类型专属字段 -->
      <FieldRow
        v-for="field in visibleFields"
        :key="field.key"
        :field="field"
        :value="event[field.key]"
        :event="event"
        :diagnostics="fieldDiagnostics(field.key)"
        @update="(v: unknown) => emitField(field.key, v)"
      />

      <!-- 通用字段 -->
      <div class="my-3
        border-t
        border-white/10
        pt-3">
        <p class="mb-2
          text-xs
          tracking-wide
          text-white/35">
          {{ t('scriptEditor.eventProperty.commonFields') }}
        </p>
        <FieldRow
          v-for="field in commonFieldsToShow"
          :key="field.key"
          :field="field"
          :value="event[field.key]"
          :event="event"
          :diagnostics="fieldDiagnostics(field.key)"
          @update="(v: unknown) => emitField(field.key, v)"
        />
      </div>

      <!-- 本事件上的诊断 -->
      <div
        v-if="eventDiagnostics.length"
        class="rounded-xl
          border
          border-white/10
          bg-black/15
          p-4"
      >
        <p class="mb-2
          text-sm
          font-semibold
          text-white">
          {{ t('scriptEditor.eventProperty.hasProblems') }}
        </p>
        <div
          v-for="(d, i) in eventDiagnostics"
          :key="i"
          class="mb-2
            text-xs
            leading-relaxed
            last:mb-0"
          :class="severityClass(d.severity)"
        >
          {{ d.message }}
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import type { Diagnostic, EventSpec, FieldSpec } from '@/api/services/script-editor'
import FieldRow from '../fields/FieldRow.vue'
import { categoryLabelOf, eventLabelOf } from '@/locales/schema-i18n'

const { t } = useI18n()
const store = useScriptEditorStore()

const event = computed(() => store.chapter?.events[store.selectedEvent])

const eventType = computed(() =>
  typeof event.value?.type === 'string' ? (event.value.type as string) : '',
)

const spec = computed<EventSpec | undefined>(() => store.eventSpecs[eventType.value])

/**
 * chapter_end 按结束方式联动显示字段：
 * - linear    → 只留 next_chapter（引擎只读 next/next_chapter，分支和 AI 提示都不看）
 * - branching → 只看分支，AI 判定提示不显示
 * - ai_judged → 分支 + AI 判定提示都显示
 * 隐藏的字段值不会丢，切换结束方式后自动回来。
 */
const visibleFields = computed<FieldSpec[]>(() => {
  const fields = spec.value?.fields ?? []
  if (eventType.value !== 'chapter_end') return fields
  const et = typeof event.value?.end_type === 'string' ? event.value.end_type : 'linear'
  return fields.filter((f) => {
    if (f.key === 'options') return et === 'branching' || et === 'ai_judged'
    if (f.key === 'prompt') return et === 'ai_judged'
    return true
  })
})

/** 按 schema 的 category 分组，与「添加事件」面板保持一致 */
const groupedSpecs = computed(() => {
  const out: Record<string, EventSpec[]> = {}
  for (const e of store.schema?.events ?? []) {
    ;(out[e.category] ||= []).push(e)
  }
  return out
})

/**
 * 通用字段：触发条件 / 事件间隔（duration）——所有事件类型共有，
 * 定义在 schema 的 common_fields，此处原样透出。
 */
const commonFieldsToShow = computed<FieldSpec[]>(() => store.schema?.commonFields ?? [])

const eventDiagnostics = computed<Diagnostic[]>(
  () => store.chapterDiagnostics[store.selectedEvent] ?? [],
)

const fieldDiagnostics = (key: string) => eventDiagnostics.value.filter((d) => d.field === key)

const severityClass = (s: string) =>
  s === 'error' ? 'text-red-300' : s === 'warn' ? 'text-yellow-200' : 'text-white/50'

const emitField = (key: string, value: unknown) => {
  store.setEventField(store.selectedEvent, key, value)
}

/**
 * 换事件类型时保留同名字段，其余丢弃。
 *
 * 直接原地改 type 会留下一堆新类型不认识的键，校验器会全部报「未知字段」，
 * 所以按新类型的 schema 过滤一遍。
 */
const onTypeChange = (e: Event) => {
  const next = (e.target as HTMLSelectElement).value
  const nextSpec = store.eventSpecs[next]
  if (!nextSpec || !event.value || !store.chapter) return

  // 按「字段名相同 **且** 控件类型相同」保留旧值。
  // 只比字段名会把 choices 的 options（[{text, actions}]）原样搬进
  // set_variable 的 options（[{condition, actions}]），语义完全不同，
  // 校验器立刻报错。复合类型之间一律不继承。
  const prevSpec = spec.value
  const prevKinds = new Map(prevSpec?.fields.map((f) => [f.key, f.kind]) ?? [])
  const nextKinds = new Map(nextSpec.fields.map((f) => [f.key, f.kind]))
  for (const f of store.schema?.commonFields ?? []) {
    prevKinds.set(f.key, f.kind)
    nextKinds.set(f.key, f.kind)
  }

  const rebuilt = store.blankEvent(next)
  for (const [k, v] of Object.entries(event.value)) {
    if (k === 'type') continue
    const nk = nextKinds.get(k)
    if (nk !== undefined && nk === prevKinds.get(k)) rebuilt[k] = v
  }

  store.replaceEvent(store.selectedEvent, rebuilt)
}
</script>
