<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Icon, Toggle } from '@/components/base'
import { MenuPage, MenuItem } from '@/components/ui'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import ChapterFlow from './ChapterFlow.vue'
import ChapterTimeline from './ChapterTimeline.vue'
import EventPropertyPanel from './EventPropertyPanel.vue'
import { openScriptFolder } from '@/api/services/script-editor'

const emit = defineEmits<{ 'new-chapter': [] }>()

const { t } = useI18n()
const store = useScriptEditorStore()

/** 抽成常量纯粹是因为 title 内联会超出 100 列的行宽 */
const FOLD_HINT = computed(() => t('scriptEditor.flowTab.foldHint'))

/**
 * 事件属性栏临时展开：默认 340px，展开时覆盖时间线（其余区域毛玻璃模糊），
 * 便于长字段的输入与全览；点遮罩或头部按钮即可回归。退出章节编辑时自动收起。
 */
const propsExpanded = ref(false)
watch(
  () => store.level,
  (level) => {
    if (level !== 'chapter') propsExpanded.value = false
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
  <div
    v-else
    class="relative
      flex
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

    <!-- 展开态遮罩：盖住并模糊时间线，点击即收起 -->
    <Transition name="props-mask">
      <div
        v-if="propsExpanded"
        class="absolute
          inset-0
          z-10
          rounded-xl
          backdrop-blur-sm
          bg-black/45"
        @click="propsExpanded = false"
      ></div>
    </Transition>

    <div
      class="relative
        z-20
        flex
        min-h-0
        flex-col
        transition-[flex-basis]
        duration-300
        ease-out"
      :class="propsExpanded ? 'flex-[0_0_min(760px,78%)]' : 'flex-[0_0_340px]'"
    >
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
          <button
            class="inline-flex
              items-center
              justify-center
              w-6
              h-6
              rounded-md
              text-[0.7rem]
              text-white/40
              transition-colors
              hover:text-brand
              hover:bg-white/10"
            :title="
              propsExpanded
                ? t('scriptEditor.flowTab.collapseProps')
                : t('scriptEditor.flowTab.expandProps')
            "
            @click="propsExpanded = !propsExpanded"
          >
            {{ propsExpanded ? '⤡' : '⤢' }}
          </button>
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

/* 遮罩淡入淡出 */
.props-mask-enter-active,
.props-mask-leave-active {
  transition: opacity 0.3s ease;
}
.props-mask-enter-from,
.props-mask-leave-to {
  opacity: 0;
}
</style>
