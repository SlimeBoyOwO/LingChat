<template>
  <div class="flex flex-col gap-1">
    <template v-if="!event">
      <p class="py-8 text-center text-sm text-white/40">在左侧选中一个事件</p>
    </template>

    <template v-else>
      <!-- 事件类型 -->
      <div class="mb-4">
        <label class="inline-flex items-center font-medium text-brand">事件类型</label>
        <p class="mt-1 mb-2 text-sm text-gray-300">type</p>
        <select
          class="glass-input"
          :value="eventType"
          @change="onTypeChange"
        >
          <optgroup
            v-for="(group, cat) in groupedSpecs"
            :key="cat"
            :label="cat"
          >
            <option
              v-for="s in group"
              :key="s.typeKey"
              :value="s.typeKey"
            >
              {{ s.label }}（{{ s.typeKey }}）
            </option>
          </optgroup>
        </select>
        <p class="mt-1 text-xs text-white/40">
          共 {{ store.schema?.events.length ?? 0 }} 种，列表由 Rust 侧 get_script_schema 提供
        </p>
      </div>

      <!-- 类型专属字段 -->
      <FieldRow
        v-for="field in spec?.fields ?? []"
        :key="field.key"
        :field="field"
        :value="event[field.key]"
        :diagnostics="fieldDiagnostics(field.key)"
        @update="(v: unknown) => emitField(field.key, v)"
      />

      <!-- 通用字段 -->
      <div class="my-3 border-t border-white/10 pt-3">
        <p class="mb-2 text-xs tracking-wide text-white/35">所有事件通用</p>
        <FieldRow
          v-for="field in commonFieldsToShow"
          :key="field.key"
          :field="field"
          :value="event[field.key]"
          :diagnostics="fieldDiagnostics(field.key)"
          @update="(v: unknown) => emitField(field.key, v)"
        />
      </div>

      <!-- 本事件上的诊断 -->
      <div
        v-if="eventDiagnostics.length"
        class="rounded-xl border border-white/10 bg-black/15 p-4"
      >
        <p class="mb-2 text-sm font-semibold text-white">这个事件有问题</p>
        <div
          v-for="(d, i) in eventDiagnostics"
          :key="i"
          class="mb-2 text-xs leading-relaxed last:mb-0"
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
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import type { Diagnostic, EventSpec, FieldSpec } from '@/api/services/script-editor'
import FieldRow from './FieldRow.vue'

const store = useScriptEditorStore()

const event = computed(() => store.chapter?.events[store.selectedEvent])

const eventType = computed(() =>
  typeof event.value?.type === 'string' ? (event.value.type as string) : '',
)

const spec = computed<EventSpec | undefined>(() => store.eventSpecs[eventType.value])

/** 按 schema 的 category 分组，与「添加事件」面板保持一致 */
const groupedSpecs = computed(() => {
  const out: Record<string, EventSpec[]> = {}
  for (const e of store.schema?.events ?? []) {
    ;(out[e.category] ||= []).push(e)
  }
  return out
})

/**
 * duration 只在事件真的带了它的时候才显示。
 * 它是遗留字段，对没写过它的事件不该出现在表单里。
 */
const commonFieldsToShow = computed<FieldSpec[]>(() =>
  (store.schema?.commonFields ?? []).filter(
    (f) => f.key !== 'duration' || event.value?.duration !== undefined,
  ),
)

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

  const keep = new Set<string>(nextSpec.fields.map((f) => f.key))
  for (const f of store.schema?.commonFields ?? []) keep.add(f.key)

  const rebuilt = store.blankEvent(next)
  for (const [k, v] of Object.entries(event.value)) {
    if (k === 'type') continue
    if (keep.has(k)) rebuilt[k] = v
  }

  store.pushHistory()
  store.chapter.events[store.selectedEvent] = rebuilt
  store.markDirty()
}
</script>

<style scoped>
@reference "tailwindcss";

.glass-input {
  @apply w-full rounded-lg border border-white/10 bg-white/10 px-3 py-2.5 text-sm text-white
    backdrop-blur-xl backdrop-saturate-150 transition-all duration-200
    focus:border-brand focus:ring-2 focus:ring-brand/20 focus:outline-none;
}
.glass-input option,
.glass-input optgroup {
  background: #16202c;
  color: #fff;
}
</style>
