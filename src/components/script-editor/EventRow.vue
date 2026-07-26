<template>
  <div class="tli">
    <span
      class="dot"
      :style="{ background: spec?.color ?? '#64748b' }"
    ></span>
    <span
      v-if="conditionText"
      class="cond"
      >若 {{ conditionText }}</span
    >

    <div
      class="evrow"
      :class="{ sel: index === store.selectedEvent }"
      @click="store.selectedEvent = index"
    >
      <span
        v-if="draggableRow"
        class="handle"
        title="拖动可以调整这条事件在章节里的位置"
        >⠿</span
      >
      <span
        v-else
        class="handle handle-locked"
        title="「章节结束」必须是最后一条，位置固定"
        >⌁</span
      >

      <span
        class="badge"
        :style="{
          color: spec?.color,
          borderColor: (spec?.color ?? '#64748b') + '55',
          background: (spec?.color ?? '#64748b') + '14',
        }"
      >
        {{ spec?.label ?? eventType }}
      </span>

      <span class="etext">
        <template
          v-for="(part, i) in highlighted"
          :key="i"
        >
          <span
            v-if="part.token"
            class="tok"
            >{{ part.text }}</span
          >
          <template v-else>{{ part.text }}</template>
        </template>
      </span>

      <span
        v-if="errorCount"
        class="flag flag-bad"
        >{{ errorCount }} 个错误</span
      >
      <span
        v-else-if="warnCount"
        class="flag flag-warn"
        >{{ warnCount }} 个提醒</span
      >
      <span
        v-if="event.duration !== undefined"
        class="flag flag-warn"
        >duration 无效</span
      >

      <button
        class="act"
        title="复制"
        @click.stop="store.duplicateEvent(index)"
      >
        ⧉
      </button>
      <button
        class="act act-del"
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
  /** 由 ChapterTimeline 决定：章节结束固定在末尾，不给拖动柄 */
  draggableRow?: boolean
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
</script>

<style scoped>
.tli {
  position: relative;
}
.dot {
  position: absolute;
  left: -21px;
  top: 12px;
  width: 9px;
  height: 9px;
  border: 2px solid #2b3a4a;
  border-radius: 50%;
}
.cond {
  position: absolute;
  left: -21px;
  top: -7px;
  border: 1px solid rgba(251, 191, 36, 0.32);
  border-radius: 3px;
  padding: 0 5px;
  font-family: ui-monospace, Menlo, monospace;
  font-size: 9px;
  white-space: nowrap;
  color: #fcd34d;
  background: rgba(251, 191, 36, 0.16);
}

.evrow {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  border: 1px solid transparent;
  border-radius: 8px;
  padding: 6px 9px;
  cursor: pointer;
  transition: all 0.15s;
}
.evrow:hover {
  background: rgba(255, 255, 255, 0.07);
}
.evrow.sel {
  border-color: rgba(121, 217, 255, 0.4);
  background: rgba(121, 217, 255, 0.12);
}

.act {
  flex: 0 0 auto;
  border-radius: 4px;
  padding: 0 3px;
  font-size: 11px;
  line-height: 1.7;
  color: rgba(255, 255, 255, 0.25);
  opacity: 0;
  transition: all 0.15s;
}
.evrow:hover .act {
  opacity: 1;
}
.act:hover {
  color: var(--accent-color);
  background: rgba(255, 255, 255, 0.1);
}

/* 拖动柄常驻但很淡 —— 藏起来的话作者根本不知道这行能拖 */
.handle {
  flex: 0 0 auto;
  cursor: grab;
  font-size: 11px;
  line-height: 1.7;
  color: rgba(255, 255, 255, 0.18);
  transition: color 0.15s;
}
.evrow:hover .handle {
  color: var(--accent-color);
}
.handle-locked {
  cursor: default;
  color: rgba(255, 255, 255, 0.12);
}
.evrow:hover .handle-locked {
  color: rgba(255, 255, 255, 0.2);
}
.act-del:hover {
  color: #fca5a5;
  background: rgba(248, 113, 113, 0.15);
}

.badge {
  flex: 0 0 auto;
  border: 1px solid;
  border-radius: 5px;
  padding: 2px 7px;
  font-size: 0.7rem;
  font-weight: 500;
  line-height: 1.5;
  white-space: nowrap;
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
.tok {
  color: var(--accent-color);
  opacity: 0.8;
}
.flag {
  flex: 0 0 auto;
  border-radius: 4px;
  padding: 1px 5px;
  font-size: 0.62rem;
  line-height: 1.6;
  white-space: nowrap;
}
.flag-bad {
  color: #fca5a5;
  border: 1px solid rgba(248, 113, 113, 0.35);
  background: rgba(248, 113, 113, 0.15);
}
.flag-warn {
  color: #fcd34d;
  border: 1px solid rgba(251, 191, 36, 0.3);
  background: rgba(251, 191, 36, 0.15);
}
</style>
