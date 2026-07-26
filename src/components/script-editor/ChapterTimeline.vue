<template>
  <div class="rail">
    <template
      v-for="row in rows"
      :key="row.key"
    >
      <!-- 复合块：默认折叠成一行 -->
      <div
        v-if="row.kind === 'group'"
        class="grp"
        :class="{ open: expanded[row.key] }"
      >
        <div
          class="ghead"
          @click="toggle(row.key)"
        >
          <span class="chev">›</span>
          <span class="badge-group">{{ row.label }}</span>
          <span class="etext">{{ row.summary }}</span>
          <span
            v-if="groupHasError(row)"
            class="flag-bad"
            >含错误</span
          >
          <span class="gcount">{{ row.to - row.from }} 个事件</span>
        </div>
        <div
          v-if="expanded[row.key]"
          class="gbody"
        >
          <div class="rail rail-nested">
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
    </template>

    <button
      class="addev"
      @click="paletteOpen = true"
    >
      ＋ 插入事件（{{ store.schema?.events.length ?? 0 }} 种全部可选，插在「章节结束」之前）
    </button>

    <!-- 事件类型选择面板 -->
    <Teleport to="body">
      <Transition name="modal">
        <div
          v-if="paletteOpen"
          class="palette-mask"
          @click.self="paletteOpen = false"
        >
          <div class="palette">
            <div class="mb-4 flex items-center gap-2 border-b-2 border-brand pb-2">
              <h4 class="font-semibold text-white">插入事件</h4>
              <button
                class="ml-auto text-white/50 transition-all hover:rotate-90 hover:text-brand"
                @click="paletteOpen = false"
              >
                ✕
              </button>
            </div>
            <div
              v-for="(group, cat) in groupedSpecs"
              :key="cat"
              class="mb-3"
            >
              <p class="mb-1.5 text-xs text-white/40">{{ cat }}</p>
              <div class="flex flex-wrap gap-2">
                <button
                  v-for="spec in group"
                  :key="spec.typeKey"
                  class="rounded-lg border px-3 py-1.5 text-sm transition-all hover:bg-white/10"
                  :style="{
                    color: spec.color,
                    borderColor: spec.color + '55',
                    background: spec.color + '14',
                  }"
                  @click="insert(spec.typeKey)"
                >
                  {{ spec.label }}
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
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import { foldEvents, groupContaining, type FoldedGroup } from '@/composables/useEventFolding'
import type { EventSpec } from '@/api/services/script-editor'
import EventRow from './EventRow.vue'

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

/**
 * 换章节先收起全部，再按当前选中项展开所在块。
 *
 * 合成一个 watcher —— 拆成两个的话「按 selectedEvent 展开」会被
 * 「换章节清空」覆盖掉（两个 watcher 按创建顺序执行）。
 */
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
.rail {
  position: relative;
  padding-left: 22px;
}
.rail::before {
  content: '';
  position: absolute;
  left: 5px;
  top: 10px;
  bottom: 10px;
  width: 1px;
  background: rgba(255, 255, 255, 0.14);
}
.rail-nested {
  padding-left: 16px;
}
.rail-nested::before {
  left: -1px;
}

.grp {
  position: relative;
  margin: 3px 0;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.13);
  border-radius: 9px;
  background: rgba(0, 0, 0, 0.16);
}
/* 复合块在时间轴上用空心菱形锚点，与单事件的实心圆区分 */
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
.ghead {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 9px;
  cursor: pointer;
  transition: background 0.15s;
}
.ghead:hover {
  background: rgba(255, 255, 255, 0.05);
}
.chev {
  width: 0.8rem;
  font-size: 0.8rem;
  color: rgba(255, 255, 255, 0.4);
  transition: transform 0.2s;
}
.grp.open .chev {
  transform: rotate(90deg);
}
.gbody {
  padding: 0 6px 6px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
}
.gcount {
  font-size: 0.66rem;
  white-space: nowrap;
  color: rgba(255, 255, 255, 0.38);
}

/* 复合块的头部长得像一条事件行，所以下面几条与 EventRow 里的同名规则形状相近。
   刻意不抽成全局类：`.badge` / `.flag` / `.etext` 这种名字放到全局 CSS 里
   撞名的概率极高，代价比抄两行样式大得多。这里只留实际用到的那一档
   （EventRow 的 .badge 需要基类是因为颜色由 :style 逐事件注入，这边是固定灰）。 */
.badge-group {
  flex: 0 0 auto;
  border: 1px solid rgba(255, 255, 255, 0.25);
  border-radius: 5px;
  padding: 2px 7px;
  font-size: 0.7rem;
  font-weight: 500;
  line-height: 1.5;
  white-space: nowrap;
  color: #cbd5e1;
  background: rgba(255, 255, 255, 0.07);
}
.etext {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-size: 0.78rem;
  line-height: 1.7;
  color: rgba(255, 255, 255, 0.72);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.flag-bad {
  flex: 0 0 auto;
  border: 1px solid rgba(248, 113, 113, 0.35);
  border-radius: 4px;
  padding: 1px 5px;
  font-size: 0.62rem;
  line-height: 1.6;
  color: #fca5a5;
  background: rgba(248, 113, 113, 0.15);
}

.addev {
  margin-top: 8px;
  margin-left: -22px;
  width: calc(100% + 22px);
  border: 1px dashed rgba(255, 255, 255, 0.18);
  border-radius: 8px;
  padding: 7px;
  font-size: 0.78rem;
  color: rgba(255, 255, 255, 0.45);
  transition: all 0.15s;
}
.addev:hover {
  border-color: var(--accent-color);
  color: var(--accent-color);
  background: rgba(121, 217, 255, 0.05);
}

.palette-mask {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  backdrop-filter: blur(5px);
  background: rgba(0, 0, 0, 0.45);
}
.palette {
  width: min(560px, 92vw);
  max-height: 80vh;
  overflow-y: auto;
  border-radius: 12px;
  padding: 15px;
  background: rgba(30, 41, 59, 0.96);
  border: 1px solid rgba(255, 255, 255, 0.125);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.35);
}
.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.2s ease;
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}
</style>
