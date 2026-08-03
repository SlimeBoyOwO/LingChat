<script setup lang="ts">
import { Icon, Toggle } from '@/components/base'
import { MenuPage, MenuItem } from '@/components/ui'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import ChapterFlow from './ChapterFlow.vue'
import ChapterTimeline from './ChapterTimeline.vue'
import EventPropertyPanel from './EventPropertyPanel.vue'
import { openScriptFolder } from '@/api/services/script-editor'

const emit = defineEmits<{ 'new-chapter': [] }>()

const store = useScriptEditorStore()

/** 抽成常量纯粹是因为 title 内联会超出 100 列的行宽 */
const FOLD_HINT =
  '官方剧本里反复出现两组固定写法：「角色退场 → 背景 → 角色出场」的转场，' +
  '和「AI 说 → 等玩家输入 → AI 说」的一轮互动。打开后它们各折成一行，' +
  '长章节能少掉近一半行数；折起来的那行会写明这段切到哪个背景、用的什么提示。'

const onRename = (e: Event) => store.setChapterName((e.target as HTMLInputElement).value)

const openFolder = async () => {
  if (!store.scriptKey) return
  try {
    await openScriptFolder(store.scriptKey)
  } catch (err) {
    store.notifyError('打开目录失败', err)
  }
}
</script>

<template>
  <!-- ============ 章节流程 ============ -->
  <MenuPage v-if="store.level === 'flow'">
    <MenuItem title="章节流程">
      <template #header>
        <Icon
          icon="adventure"
          :size="20"
        />
      </template>
      <div class="flex flex-wrap items-center gap-2 mb-3">
        <button
          class="inline-flex items-center gap-1 border border-white/10 rounded-lg px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap text-white/70 bg-white/6 transition-all duration-200 hover:enabled:text-white hover:enabled:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40"
          @click="emit('new-chapter')"
        >
          ＋ 新建章节
        </button>
        <button
          class="inline-flex items-center gap-1 border border-white/10 rounded-lg px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap text-white/70 bg-white/6 transition-all duration-200 hover:enabled:text-white hover:enabled:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40"
          @click="store.runValidation()"
        >
          重新校验
        </button>
        <button
          class="inline-flex items-center gap-1 border border-white/10 rounded-lg px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap text-white/70 bg-white/6 transition-all duration-200 hover:enabled:text-white hover:enabled:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40"
          @click="openFolder"
        >
          打开剧本目录
        </button>
      </div>
      <ChapterFlow />
    </MenuItem>
  </MenuPage>

  <!-- ============ 章节编辑 ============ -->
  <div
    v-else
    class="flex w-[94%] min-h-0 flex-1 gap-5 mx-auto px-3 py-4"
  >
    <div class="flex min-w-0 flex-1 flex-col">
      <MenuItem
        title="事件时间线"
        class="fill flex h-full min-h-0 flex-col"
      >
        <template #header>
          <Icon
            icon="text"
            :size="20"
          />
        </template>
        <div class="mb-2 flex items-center gap-2">
          <input
            class="glass-input flex-1"
            placeholder="章节显示名（留空则用文件名）"
            :value="store.chapter?.name ?? ''"
            @change="onRename"
          />
          <label
            class="inline-flex items-center gap-2 text-[0.8rem] whitespace-nowrap text-white/70"
            :title="FOLD_HINT"
          >
            <Toggle
              :checked="store.foldCompounds"
              @change="(v: boolean) => (store.foldCompounds = v)"
            />
            合并转场等固定组合
          </label>
          <span class="shrink-0 text-xs text-white/40">
            {{ store.chapter?.events.length ?? 0 }} 个事件
          </span>
        </div>
        <div class="min-h-0 flex-1 overflow-y-auto pr-1">
          <ChapterTimeline />
        </div>
      </MenuItem>
    </div>

    <div class="flex min-h-0 flex-[0_0_340px] flex-col">
      <MenuItem
        title="事件属性"
        class="fill flex h-full min-h-0 flex-col"
      >
        <template #header>
          <Icon
            icon="setting"
            :size="20"
          />
        </template>
        <div class="min-h-0 flex-1 overflow-y-auto pr-1">
          <EventPropertyPanel />
        </div>
      </MenuItem>
    </div>
  </div>
</template>

<style scoped>
/* MenuItem 的 .content 默认只有 width:100%，在 .fill（flex 列）里不会收缩 */
.fill :deep(.content) {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
</style>
