<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Icon, Toggle } from '@/components/base'
import { MenuPage, MenuItem } from '@/components/ui'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import ChapterFlow from '../flow/ChapterFlow.vue'
import ChapterTimeline from '../flow/ChapterTimeline.vue'
import EventPropertyPanel from '../flow/EventPropertyPanel.vue'
import { openScriptFolder } from '@/api/services/script-editor'
import { isAndroid } from '@/utils/platform'

const emit = defineEmits<{ 'new-chapter': [] }>()

const { t } = useI18n()
const store = useScriptEditorStore()

/** 抽成常量纯粹是因为 title 内联会超出 100 列的行宽 */
const FOLD_HINT = computed(() => t('scriptEditor.flowTab.foldHint'))

/** 属性栏拖拽宽度钳制：最小 360px，最大不超过编辑器宽度的 88% */
const GRIP_MIN = 360
const GRIP_MAX_RATIO = 0.88

// 章节编辑容器 ref，拖拽时取它的宽度计算上限
const editorWrap = ref<HTMLElement | null>(null)

/**
 * 属性栏边缘竖条手柄：单击展开/折叠，按住拖拽调宽度。
 * 用 pointer 事件区分单击与拖拽（移动超 4px 视为拖拽），拖拽结果写入
 * store.propsWidth（持久化），展开/折叠状态是临时态不持久化。
 */
let gripStartX = 0
let gripStartW = 0
let gripDragging = false

const onGripDown = (e: PointerEvent) => {
  gripStartX = e.clientX
  gripStartW = store.propsExpanded ? store.propsWidth : 340
  gripDragging = false
  const onMove = (ev: PointerEvent) => {
    const delta = gripStartX - ev.clientX
    if (!gripDragging && Math.abs(delta) > 4) gripDragging = true
    if (!gripDragging) return
    const wrap = editorWrap.value
    const maxW = wrap ? Math.max(GRIP_MIN, Math.floor(wrap.clientWidth * GRIP_MAX_RATIO)) : 1200
    store.propsExpanded = true
    store.propsWidth = Math.min(maxW, Math.max(GRIP_MIN, gripStartW + delta))
  }
  const onUp = () => {
    window.removeEventListener('pointermove', onMove)
    window.removeEventListener('pointerup', onUp)
    // 没有进入拖拽就是单击 → 切换展开/折叠
    if (!gripDragging) store.propsExpanded = !store.propsExpanded
  }
  window.addEventListener('pointermove', onMove)
  window.addEventListener('pointerup', onUp)
}

/** 退出章节编辑时自动收起属性栏 */
watch(
  () => store.level,
  (level) => {
    if (level !== 'chapter') store.propsExpanded = false
  },
)

const onRename = (e: Event) => store.setChapterName((e.target as HTMLInputElement).value)

const openFolder = async () => {
  if (!store.scriptKey) return
  try {
    await openScriptFolder(store.scriptKey)
  } catch (err) {
    store.notifyError(t('scriptEditor.notify.openFolderFailed'), err)
  }
}
</script>

<template>
  <!-- ============ 章节流程 ============ -->
  <MenuPage v-if="store.level === 'flow'">
    <MenuItem :title="t('scriptEditor.flowTab.menuTitle')">
      <template #header>
        <Icon
          icon="adventure"
          :size="20"
        />
      </template>
      <div class="flex
        flex-wrap
        items-center
        gap-2
        mb-3">
        <button
          class="inline-flex
            items-center
            gap-1
            border
            border-white/10
            rounded-lg
            px-3
            py-[0.3rem]
            text-[0.8rem]
            whitespace-nowrap
            text-white/70
            bg-white/6
            transition-all
            duration-200
            hover:enabled:text-white
            hover:enabled:bg-white/[0.12]
            disabled:cursor-not-allowed
            disabled:opacity-40"
          @click="emit('new-chapter')"
        >
          {{ t('scriptEditor.flowTab.newChapter') }}
        </button>
        <button
          class="inline-flex
            items-center
            gap-1
            border
            border-white/10
            rounded-lg
            px-3
            py-[0.3rem]
            text-[0.8rem]
            whitespace-nowrap
            text-white/70
            bg-white/6
            transition-all
            duration-200
            hover:enabled:text-white
            hover:enabled:bg-white/[0.12]
            disabled:cursor-not-allowed
            disabled:opacity-40"
          @click="store.runValidation()"
        >
          {{ t('scriptEditor.validate.revalidate') }}
        </button>
        <button
          v-if="!isAndroid()"
          class="inline-flex
            items-center
            gap-1
            border
            border-white/10
            rounded-lg
            px-3
            py-[0.3rem]
            text-[0.8rem]
            whitespace-nowrap
            text-white/70
            bg-white/6
            transition-all
            duration-200
            hover:enabled:text-white
            hover:enabled:bg-white/[0.12]
            disabled:cursor-not-allowed
            disabled:opacity-40"
          @click="openFolder"
        >
          {{ t('scriptEditor.flowTab.openFolder') }}
        </button>
      </div>
      <ChapterFlow />
    </MenuItem>
  </MenuPage>

  <!-- ============ 章节编辑 ============ -->
  <!-- 注意：这里不能加 relative —— 组件根元素会收到 <component> fallthrough 的
       absolute inset-0，两个 position 类并存时 CSS 里 relative 在后会胜出，
       元素从「撑满容器」塌缩为「内容高度」，内部滚动链就断了 -->
  <div
    v-else
    ref="editorWrap"
    class="flex
      w-[94%]
      min-h-0
      flex-1
      gap-5
      mx-auto
      px-3
      py-4"
  >
    <div class="flex
      min-w-0
      flex-1
      flex-col">
      <MenuItem
        :title="t('scriptEditor.flowTab.timeline')"
        class="fill
          flex
          h-full
          min-h-0
          flex-col"
      >
        <template #header>
          <Icon
            icon="text"
            :size="20"
          />
        </template>
        <div class="mb-2
          flex
          items-center
          gap-2">
          <input
            class="glass-input
              flex-1"
            :placeholder="t('scriptEditor.flowTab.chapterName')"
            :value="store.chapter?.name ?? ''"
            @change="onRename"
          />
          <label
            class="inline-flex
              items-center
              gap-2
              text-[0.8rem]
              whitespace-nowrap
              text-white/70"
            :title="FOLD_HINT"
          >
            <Toggle
              :checked="store.foldCompounds"
              @change="(v: boolean) => (store.foldCompounds = v)"
            />
            {{ t('scriptEditor.flowTab.foldToggle') }}
          </label>
          <span class="shrink-0
            text-xs
            text-white/40">
            {{ t('scriptEditor.chapterFlow.events', { count: store.chapter?.events.length ?? 0 }) }}
          </span>
        </div>
        <div class="min-h-0
          flex-1
          overflow-y-auto
          pr-1">
          <ChapterTimeline />
        </div>
      </MenuItem>
    </div>

    <!-- 属性栏：展开时不遮挡时间线（并行查看），宽度由边缘手柄拖拽记忆 -->
    <div
      class="relative
        flex
        min-h-0
        flex-col
        transition-[flex-basis]
        duration-300
        ease-out"
      :style="store.propsExpanded ? { flexBasis: `${store.propsWidth}px` } : { flexBasis: '340px' }"
    >
      <!-- 边缘竖条手柄：单击展开/折叠，按住拖拽调宽度 -->
      <div
        class="group/grip
          absolute
          left-0
          top-0
          bottom-0
          z-30
          w-2
          -translate-x-1/2
          cursor-ew-resize
          touch-none"
        :title="t('scriptEditor.flowTab.propsGrip')"
        @pointerdown="onGripDown"
      >
        <div
          class="absolute
            left-1/2
            top-1/2
            h-20
            w-[3px]
            -translate-x-1/2
            -translate-y-1/2
            rounded-full
            bg-white/20
            transition-colors
            group-hover/grip:bg-brand"
        ></div>
      </div>
      <MenuItem
        :title="t('scriptEditor.flowTab.eventProps')"
        class="fill
          flex
          h-full
          min-h-0
          flex-col"
      >
        <template #header>
          <Icon
            icon="setting"
            :size="20"
          />
        </template>
        <div class="min-h-0
          flex-1
          overflow-y-auto
          pr-1">
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
