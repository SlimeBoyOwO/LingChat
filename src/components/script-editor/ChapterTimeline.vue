<template>
  <div class="rail
    relative
    pl-[22px]">
    <template
      v-for="(row, ri) in rows"
      :key="row.key"
    >
      <div class="min-w-0
        flex-1">
        <!-- 复合块：默认折叠成一行 -->
        <div
          v-if="row.kind === 'group'"
          class="grp
            group
            relative
            my-[3px]
            overflow-hidden
            border
            border-white/[0.13]
            rounded-[9px]
            bg-black/16"
          :class="{ open: expanded[row.key] }"
        >
          <div
            class="flex
              items-center
              gap-2
              px-[9px]
              py-[7px]
              cursor-pointer
              transition-colors
              duration-150
              hover:bg-white/5"
            @click="toggle(row.key)"
          >
            <span
              class="w-3.5
                text-[0.8rem]
                text-white/40
                transition-transform
                duration-200
                group-[.open]:rotate-90"
              >›</span
            >
            <span
              class="shrink-0
                border
                border-white/25
                rounded-[5px]
                px-[7px]
                py-0.5
                text-[0.7rem]
                font-medium
                leading-[1.5]
                whitespace-nowrap
                text-slate-300
                bg-white/7"
              >{{ row.label }}</span
            >
            <span
              class="flex-1
                min-w-0
                overflow-hidden
                text-[0.78rem]
                leading-[1.7]
                text-white/[0.72]
                truncate"
              >{{ row.summary }}</span
            >
            <span
              v-if="groupHasError(row)"
              class="shrink-0
                border
                border-red-400/35
                rounded
                px-[5px]
                py-px
                text-[0.62rem]
                leading-[1.6]
                whitespace-nowrap
                text-red-300
                bg-red-400/15"
              >{{ t('scriptEditor.chapterTimeline.hasError') }}</span
            >
            <span class="text-[0.66rem]
              whitespace-nowrap
              text-white/[0.38]">{{
              t('scriptEditor.chapterTimeline.events', { count: row.to - row.from })
            }}</span>
            <!-- 复合块整段移动：与 EventRow 的 ▲▼ 同一套操作语义（moveEventRange） -->
            <button
              v-if="canMoveUp(row)"
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
              @click.stop="moveUp(row)"
            >
              ▲
            </button>
            <button
              v-if="canMoveDown(row)"
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
              @click.stop="moveDown(row)"
            >
              ▼
            </button>
          </div>
          <div
            v-if="expanded[row.key]"
            class="px-1.5
              pb-1.5
              border-t
              border-white/[0.08]"
          >
            <div class="rail-nested
              pl-4">
              <EventRow
                v-for="item in row.items"
                :key="item.index"
                :index="item.index"
                :event="item.event"
              />
            </div>
          </div>
        </div>

        <!-- 单个事件 -->
        <EventRow
          v-else
          :index="row.index"
          :event="row.event"
        />
      </div>
    </template>

    <button
      class="mt-2
        -ml-[22px]
        w-[calc(100%+22px)]
        border
        border-dashed
        border-white/18
        rounded-lg
        p-[7px]
        text-[0.78rem]
        text-white/45
        transition-all
        duration-150
        hover:border-brand
        hover:text-brand
        hover:bg-[rgba(121,217,255,0.05)]"
      @click="paletteOpen = true"
    >
      {{
        t('scriptEditor.chapterTimeline.insertEvent', {
          count: store.schema?.events.length ?? 0,
        })
      }}
    </button>

    <!-- 事件类型选择面板 -->
    <Teleport to="body">
      <Transition
        enter-active-class="transition-opacity duration-200 ease"
        leave-active-class="transition-opacity duration-200 ease"
        enter-from-class="opacity-0"
        leave-to-class="opacity-0"
      >
        <div
          v-if="paletteOpen"
          class="modal-mask
            fixed
            inset-0
            z-[9999]
            flex
            items-center
            justify-center
            backdrop-blur-md
            bg-black/55"
          @click.self="paletteOpen = false"
        >
          <div
            class="w-[min(560px,92vw)]
              max-h-[80vh]
              overflow-y-auto
              border
              border-white/12.5
              rounded-xl
              py-4
              px-[18px]
              pb-[18px]
              bg-[rgba(12,20,30,0.86)]
              backdrop-blur-lg
              backdrop-saturate-[1.4]
              shadow-[0_8px_32px_rgba(0,0,0,0.45),inset_0_1px_1px_rgba(255,255,255,0.06)]"
          >
            <div class="flex
              items-center
              mb-3.5
              pb-2
              border-b-2
              border-brand">
              <h4 class="text-[0.95rem]
                font-semibold
                text-white">
                {{ t('scriptEditor.chapterTimeline.insertEventTitle') }}
              </h4>
              <button
                class="ml-auto
                  text-[0.85rem]
                  text-white/50
                  cursor-pointer
                  transition-all
                  duration-200
                  hover:text-brand
                  hover:rotate-90"
                @click="paletteOpen = false"
              >
                ✕
              </button>
            </div>
            <div
              v-for="(group, cat) in groupedSpecs"
              :key="cat"
              class="mt-3.5
                first:mt-0"
            >
              <p class="mb-[7px]
                text-[0.7rem]
                tracking-[0.5px]
                text-white/[0.38]">
                {{ categoryLabelOf(cat) }}
              </p>
              <div class="grid
                grid-cols-[repeat(auto-fill,minmax(104px,1fr))]
                gap-[7px]">
                <button
                  v-for="spec in group"
                  :key="spec.typeKey"
                  class="border
                    border-white/10
                    rounded-lg
                    px-2.5
                    py-2
                    text-[0.8rem]
                    text-white/[0.78]
                    bg-white/5
                    transition-all
                    duration-150
                    ease-in-out
                    hover:border-brand
                    hover:text-white
                    hover:bg-[rgba(121,217,255,0.14)]
                    hover:-translate-y-px"
                  @click="insert(spec.typeKey)"
                >
                  {{ eventLabelOf(spec) }}
                </button>
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import {
  foldEvents,
  groupContaining,
  type FoldedGroup,
  type FoldedRow,
} from '@/composables/useEventFolding'
import type { EventSpec } from '@/api/services/script-editor'
import EventRow from './EventRow.vue'
import { categoryLabelOf, eventLabelOf } from '@/locales/schema-i18n'

const { t } = useI18n()
const store = useScriptEditorStore()

const paletteOpen = ref(false)
const expanded = ref<Record<string, boolean>>({})

const rows = computed(() => foldEvents(store.chapter?.events ?? [], store.foldCompounds))

const groupedSpecs = computed(() => {
  const out: Record<string, EventSpec[]> = {}
  for (const e of store.schema?.events ?? []) {
    ;(out[e.category] ||= []).push(e)
  }
  return out
})

// ---- 复合块整段移动（▲▼） ----

const typeAt = (i: number) => {
  const t = store.chapter?.events[i]?.type
  return typeof t === 'string' ? t : ''
}

const rowStart = (row: FoldedRow) => (row.kind === 'group' ? row.from : row.index)
const rowSpan = (row: FoldedRow) => (row.kind === 'group' ? row.to - row.from : 1)

const canMoveUp = (row: FoldedRow) => rowStart(row) > 0

const canMoveDown = (row: FoldedRow) => {
  const end = rowStart(row) + rowSpan(row)
  const total = store.chapter?.events.length ?? 0
  // 不能越过 chapter_end
  const lastIdx = total > 0 && typeAt(total - 1) === 'chapter_end' ? total - 1 : total
  return end < lastIdx
}

const moveUp = (row: FoldedRow) => {
  const from = rowStart(row)
  store.moveEventRange(from, rowSpan(row), from - 1)
}

const moveDown = (row: FoldedRow) => {
  const from = rowStart(row)
  store.moveEventRange(from, rowSpan(row), from + 1)
}

// ---- 分组折叠 ----

watch(
  [() => store.chapter?.id, () => store.selectedEvent],
  ([id], [prevId]) => {
    if (id !== prevId) expanded.value = {}
    const gi = groupContaining(rows.value, store.selectedEvent)
    if (gi !== null) {
      const key = rows.value[gi]?.key
      if (key && !expanded.value[key]) expanded.value = { ...expanded.value, [key]: true }
    }
  },
  { immediate: true },
)

const toggle = (key: string) => {
  expanded.value = { ...expanded.value, [key]: !expanded.value[key] }
}

const groupHasError = (row: FoldedGroup) => {
  for (let i = row.from; i < row.to; i++) {
    if ((store.chapterDiagnostics[i] ?? []).some((d) => d.severity === 'error')) return true
  }
  return false
}

const insert = (typeKey: string) => {
  store.insertEvent(typeKey)
  paletteOpen.value = false
}
</script>

<style scoped>
/* 伪元素无法用 Tailwind 工具类表达，保留在 scoped 块中 */
/* 时间轴竖线 */
.rail::before {
  content: '';
  position: absolute;
  left: 5px;
  top: 10px;
  bottom: 10px;
  width: 1px;
  background: rgba(255, 255, 255, 0.14);
}
/* 嵌套时间轴偏移 */
.rail-nested::before {
  left: -1px;
}
/* 复合块在时间轴上的菱形锚点 */
.grp::before {
  content: '';
  position: absolute;
  left: -21px;
  top: 14px;
  width: 9px;
  height: 9px;
  background: #2b3a4a;
  border: 2px solid #94a3b8;
  transform: rotate(45deg);
}
</style>
