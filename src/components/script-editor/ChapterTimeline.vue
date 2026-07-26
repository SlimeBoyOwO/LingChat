<template>
  <div
    class="rail"
    @dragend="endDrag"
  >
    <template
      v-for="(row, ri) in rows"
      :key="row.key"
    >
      <!-- 插入位标记。落点一律是「插到这一行前面」，不按指针在上半还是下半区分：
           useZoom 的 CSS zoom 会让 getBoundingClientRect 和鼠标坐标对不上，
           而「悬停哪行就插哪行前面」根本不需要坐标。 -->
      <div
        class="slot"
        :class="{ on: dropAt === rowStart(row) && dragging !== null }"
        @dragover.prevent="dropAt = rowStart(row)"
        @drop.prevent="finishDrag"
      ></div>

      <div
        class="draggable"
        :class="{ ghost: isDragged(row) }"
        :draggable="canDrag(row)"
        @dragstart="startDrag(row, $event)"
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
            <span
              class="handle"
              title="拖动可以调整这一段在章节里的位置"
              >⠿</span
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
          :draggable-row="canDrag(row)"
        />
      </div>
    </template>

    <!-- 末尾落点。有「章节结束」时插在它前面 —— 它必须留在最后一条 -->
    <div
      class="slot"
      :class="{ on: dropAt === tailIndex && dragging !== null }"
      @dragover.prevent="dropAt = tailIndex"
      @drop.prevent="finishDrag"
    ></div>

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
            <div class="phead">
              <h4>插入事件</h4>
              <button
                class="pclose"
                @click="paletteOpen = false"
              >
                ✕
              </button>
            </div>
            <div
              v-for="(group, cat) in groupedSpecs"
              :key="cat"
              class="pcat"
            >
              <p class="pcat-title">{{ cat }}</p>
              <div class="pgrid">
                <button
                  v-for="spec in group"
                  :key="spec.typeKey"
                  class="pitem"
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
import {
  foldEvents,
  groupContaining,
  type FoldedGroup,
  type FoldedRow,
} from '@/composables/useEventFolding'
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

// ============================================================
// 拖拽排序
// ============================================================

/** 一行覆盖的事件区间 */
const rowStart = (row: FoldedRow) => (row.kind === 'group' ? row.from : row.index)
const rowSpan = (row: FoldedRow) => (row.kind === 'group' ? row.to - row.from : 1)

const typeAt = (i: number) => {
  const t = store.chapter?.events[i]?.type
  return typeof t === 'string' ? t : ''
}

/**
 * 末尾落点的下标：有「章节结束」就插在它前面。
 *
 * 引擎按顺序执行到 chapter_end 就跳走，排在它后面的事件永远跑不到 —— 校验器
 * 会报这条，但更好的做法是压根不给作者把东西拖到那儿的机会。
 */
const tailIndex = computed(() => {
  const list = store.chapter?.events ?? []
  const last = list.length - 1
  return last >= 0 && typeAt(last) === 'chapter_end' ? last : list.length
})

/** 章节结束固定在最后一条，不参与拖拽 */
const canDrag = (row: FoldedRow) =>
  !(row.kind === 'event' && typeAt(row.index) === 'chapter_end')

const dragging = ref<{ from: number; count: number } | null>(null)
const dropAt = ref<number | null>(null)

const isDragged = (row: FoldedRow) => dragging.value?.from === rowStart(row)

const startDrag = (row: FoldedRow, e: DragEvent) => {
  if (!canDrag(row)) return
  dragging.value = { from: rowStart(row), count: rowSpan(row) }
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('text/plain', String(rowStart(row)))
  }
}

const endDrag = () => {
  dragging.value = null
  dropAt.value = null
}

const finishDrag = () => {
  const d = dragging.value
  const at = dropAt.value
  endDrag()
  if (!d || at === null) return
  store.moveEventRange(d.from, d.count, Math.min(at, tailIndex.value))
}

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

/* 落点。平时是 2px 的透明缝，拖动中才亮起来，所以静止时完全看不见它 */
.slot {
  height: 2px;
  margin: 1px 0;
  border-radius: 2px;
  transition: all 0.12s;
}
.slot.on {
  height: 6px;
  background: var(--accent-color);
  box-shadow: 0 0 8px rgba(121, 217, 255, 0.6);
}
.draggable.ghost {
  opacity: 0.35;
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
.handle {
  cursor: grab;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.3);
}
.ghead:hover .handle {
  color: var(--accent-color);
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

/* ---- 插入事件面板 ----
   与编辑器其余部分同一套：深色玻璃底 + brand 下划线标题 + 悬停高亮。
   按钮不再按事件类型上色 —— 十几个饱和色块摆在一起既吵又没有信息量，
   分类标题已经把「这是哪一类」说清楚了。 */
.palette-mask {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  backdrop-filter: blur(6px);
  background: rgba(0, 0, 0, 0.55);
}
.palette {
  width: min(560px, 92vw);
  max-height: 80vh;
  overflow-y: auto;
  border: 1px solid rgba(255, 255, 255, 0.125);
  border-radius: 12px;
  padding: 16px 18px 18px;
  background: rgba(12, 20, 30, 0.94);
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.45),
    inset 0 1px 1px rgba(255, 255, 255, 0.06);
}
.phead {
  display: flex;
  align-items: center;
  margin-bottom: 14px;
  padding-bottom: 8px;
  border-bottom: 2px solid var(--accent-color);
}
.phead h4 {
  font-size: 0.95rem;
  font-weight: 600;
  color: #fff;
}
.pclose {
  margin-left: auto;
  font-size: 0.85rem;
  color: rgba(255, 255, 255, 0.5);
  transition: all 0.2s;
}
.pclose:hover {
  color: var(--accent-color);
  transform: rotate(90deg);
}
.pcat + .pcat {
  margin-top: 14px;
}
.pcat-title {
  margin-bottom: 7px;
  font-size: 0.7rem;
  letter-spacing: 0.5px;
  color: rgba(255, 255, 255, 0.38);
}
.pgrid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
  gap: 7px;
}
.pitem {
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  padding: 8px 10px;
  font-size: 0.8rem;
  color: rgba(255, 255, 255, 0.78);
  background: rgba(255, 255, 255, 0.05);
  transition: all 0.15s ease-in-out;
}
.pitem:hover {
  border-color: var(--accent-color);
  color: #fff;
  background: rgba(121, 217, 255, 0.14);
  transform: translateY(-1px);
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
