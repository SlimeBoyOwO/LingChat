<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button, Icon } from '@/components/base'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import { useSettingsStore } from '@/stores/modules/settings'
import { formatBinding } from '@/utils/shortcuts'

const { t } = useI18n()

const emit = defineEmits<{
  playtest: []
  'toggle-shortcut-help': []
  leave: []
}>()

const store = useScriptEditorStore()
const settings = useSettingsStore()

type TabKey =
  | 'flow'
  | 'config'
  | 'characters'
  | 'assets'
  | 'validate'
  | 'agent-chat'
  | 'agent-settings'
  | 'appearance'

const tabs: {
  key: TabKey
  label: string
  icon:
    | 'adventure'
    | 'setting'
    | 'character'
    | 'background'
    | 'achievement'
    | 'bot'
    | 'sliders'
    | 'palette'
}[] = [
  { key: 'flow', label: 'scriptEditor.editorHeader.tabFlow', icon: 'adventure' },
  { key: 'config', label: 'scriptEditor.editorHeader.tabConfig', icon: 'setting' },
  { key: 'characters', label: 'scriptEditor.editorHeader.tabCharacters', icon: 'character' },
  { key: 'assets', label: 'scriptEditor.editorHeader.tabAssets', icon: 'background' },
  { key: 'validate', label: 'scriptEditor.editorHeader.tabValidate', icon: 'achievement' },
  { key: 'agent-chat', label: 'scriptEditor.editorHeader.tabAgentChat', icon: 'bot' },
  { key: 'agent-settings', label: 'scriptEditor.editorHeader.tabAgentSettings', icon: 'sliders' },
  { key: 'appearance', label: 'scriptEditor.editorHeader.tabAppearance', icon: 'palette' },
]

// ---- nav 指示条（与 SettingsNav 同一套做法）----
const navEl = ref<HTMLElement | null>(null)
const indicatorEl = ref<HTMLElement | null>(null)
const tabRefs: Record<string, HTMLElement | null> = {}

const setTabRef = (key: string, el: unknown) => {
  tabRefs[key] = (el as { $el?: HTMLElement } | null)?.$el ?? null
}

/**
 * 指示条定位。
 *
 * 之前它经常停在空白处，原因是位置只在「切标签」和「换剧本」时算一次，而
 * 按钮宽度会在别的时刻变：校验跑完后「校验」上多一个错误数角标、窗口跨过
 * xl 断点时文字标签整体显隐、字体加载完成后每个字的宽度都变。nav 是
 * `justify-content: center`，任何一个按钮变宽都会把**所有**按钮推走，于是
 * 上一次算出来的 left 就落到了两个按钮中间的空当里。
 *
 * 所以这里不再指望「在正确的时刻算一次」，而是让尺寸变化自己来触发重算：
 * ResizeObserver 同时盯着 nav 和每一个按钮。另外用 getBoundingClientRect
 * 相对 nav 求差而不是 offsetLeft —— 后者依赖 offsetParent 恰好是 nav，
 * 一旦有人给中间层加了 position 就会静默偏移。
 */
const moveIndicator = async (animate = true) => {
  await nextTick()
  const bar = indicatorEl.value
  const nav = navEl.value
  if (!bar || !nav) return
  const target = tabRefs[store.tab]
  bar.style.transition = animate
    ? 'left 0.3s cubic-bezier(0.18, 0.89, 0.32, 1), width 0.3s cubic-bezier(0.18, 0.89, 0.32, 1)'
    : 'none'
  if (!target) {
    // 目标不在了就收起来。早先这里是直接 return，于是指示条保持在上一次的
    // 位置不动 —— 那正是「出现在空白处」最刺眼的一种。
    bar.style.width = '0px'
    return
  }
  const navBox = nav.getBoundingClientRect()
  const box = target.getBoundingClientRect()
  bar.style.left = `${box.left - navBox.left + nav.scrollLeft}px`
  bar.style.width = `${box.width}px`
}

watch(
  () => store.tab,
  () => moveIndicator(),
)
watch(
  () => store.detail?.package.key,
  () => moveIndicator(),
)

let navObserver: ResizeObserver | null = null

const observeNav = () => {
  if (typeof ResizeObserver === 'undefined' || !navEl.value) return
  // 不加动画：这类重算是「布局变了跟着修正」，滑一下反而像在乱动
  navObserver = new ResizeObserver(() => void moveIndicator(false))
  navObserver.observe(navEl.value)
  for (const el of Object.values(tabRefs)) if (el) navObserver.observe(el)
}

const switchTab = (key: TabKey) => {
  if (!store.detail && !['flow', 'agent-chat', 'agent-settings', 'appearance'].includes(key)) return
  store.tab = key
  if (key === 'validate') void store.runValidation()
  if (key === 'assets') {
    void store.refreshGlobalAssets()
    void store.refreshAssetFiles()
  }
  if (key === 'characters') void store.refreshGlobalCharacters()
  // 回到流程图时强制走一遍「落盘 → 重新校验」，图上画的才是磁盘里的真状态
  if (key === 'flow' && store.level === 'flow') void store.backToFlow()
}

// ---- 面包屑 ----
const saveLabel = computed(() => {
  if (store.saving) return t('scriptEditor.editorHeader.saving')
  if (store.dirty) return t('scriptEditor.editorHeader.dirty')
  if (store.lastSavedAt) {
    const d = new Date(store.lastSavedAt)
    const time = `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
    return t('scriptEditor.editorHeader.savedAt', { time })
  }
  return t('scriptEditor.editorHeader.saved')
})

onMounted(async () => {
  await moveIndicator(false)
  observeNav()
})

onUnmounted(() => {
  navObserver?.disconnect()
  navObserver = null
})
</script>

<template>
  <div>
    <!-- 顶栏：与 SettingsNav 同构 -->
    <div class="flex
      w-full
      items-center
      justify-between
      px-5
      py-2">
      <span class="ml-5
        text-[0.95rem]
        font-bold
        tracking-[0.5px]
        text-brand
        whitespace-nowrap">{{
        t('scriptEditor.editorHeader.title')
      }}</span>
      <nav
        ref="navEl"
        class="relative
          flex
          h-full
          w-full
          flex-nowrap
          items-center
          justify-center
          gap-1
          overflow-x-auto
          overflow-y-hidden
          px-2"
      >
        <div
          ref="indicatorEl"
          class="absolute
            bottom-0
            left-0
            z-10
            h-1
            w-0
            rounded-sm
            bg-brand
            shadow-[0_0_10px_rgba(121,217,255,0.4)]"
        ></div>
        <Button
          v-for="tab in tabs"
          :key="tab.key"
          :ref="(el: unknown) => setTabRef(tab.key, el)"
          type="nav"
          :icon="tab.icon"
          :active="store.tab === tab.key"
          :disabled="
            !store.detail &&
            !['flow', 'agent-chat', 'agent-settings', 'appearance'].includes(tab.key)
          "
          @click="switchTab(tab.key)"
        >
          <p class="hidden
            xl:block">{{ t(tab.label) }}</p>
          <span
            v-if="tab.key === 'validate' && store.report && store.report.errorCount > 0"
            class="ml-1
              rounded-full
              px-[5px]
              text-[0.6rem]
              text-white
              bg-red-500"
            >{{ store.report.errorCount }}</span
          >
        </Button>
      </nav>
      <Icon
        icon="close"
        :size="40"
        class="flex
          items-center
          justify-center
          rounded-full
          p-1.5
          text-white
          cursor-pointer
          transition-all
          duration-300
          ease-in-out
          hover:text-brand
          hover:bg-white/10
          hover:rotate-90"
        @click="emit('leave')"
      />
    </div>

    <!-- 面包屑 -->
    <div class="flex
      items-center
      gap-2
      px-8
      pb-1
      text-[0.8rem]
      text-white/55">
      <button
        v-if="store.detail"
        class="text-brand
          hover:underline"
        @click="store.closeScript()"
      >
        {{ t('scriptEditor.editorHeader.backToList') }}
      </button>
      <span v-else>{{ t('scriptEditor.editorHeader.home') }}</span>

      <template v-if="store.detail">
        <span class="opacity-40">›</span>
        <button
          v-if="store.level === 'chapter'"
          class="text-brand
            hover:underline"
          @click="store.backToFlow()"
        >
          {{ store.detail.package.scriptName }}
        </button>
        <b
          v-else
          class="font-semibold
            text-white"
          >{{ store.detail.package.scriptName }}</b
        >

        <template v-if="store.level === 'chapter' && store.chapter">
          <span class="opacity-40">›</span>
          <b class="font-semibold
            text-white">{{ store.chapter.name || store.chapter.id }}</b>
          <span class="text-[0.72rem]
            opacity-35">{{ store.chapter.id }}.yaml</span>
        </template>
      </template>

      <span class="flex
        items-center
        gap-3
        ml-auto">
        <span
          v-if="store.detail"
          class="inline-flex
            items-center
            gap-[5px]
            text-[0.75rem]
            text-white/50"
        >
          <i
            class="inline-block
              w-1.5
              h-1.5
              rounded-full"
            :class="store.dirty ? 'bg-amber-300' : 'bg-green-400'"
          ></i>
          {{ saveLabel }}
        </span>
        <template v-if="store.level === 'chapter'">
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
            :disabled="!store.canUndo"
            :title="`${t('scriptEditor.editorHeader.undo')} (${formatBinding(settings.shortcuts.undo)})`"
            @click="store.undo()"
          >
            ↩ {{ t('scriptEditor.editorHeader.undo') }}
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
            :disabled="!store.canRedo"
            :title="`${t('scriptEditor.editorHeader.redo')} (${formatBinding(settings.shortcuts.redo)})`"
            @click="store.redo()"
          >
            ↪ {{ t('scriptEditor.editorHeader.redo') }}
          </button>
        </template>
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
          :title="`${t('scriptEditor.editorHeader.shortcut')} (${formatBinding(settings.shortcuts.shortcutHelp)})`"
          @click="emit('toggle-shortcut-help')"
        >
          {{ t('scriptEditor.editorHeader.shortcut') }}
        </button>
        <template v-if="store.detail">
          <button
            class="inline-flex
              items-center
              gap-1
              rounded-lg
              px-3
              py-[0.3rem]
              text-[0.8rem]
              whitespace-nowrap
              transition-all
              duration-200
              border
              border-brand/45
              text-brand
              bg-brand/14
              hover:bg-brand/24"
            :title="formatBinding(settings.shortcuts.playtest)"
            @click="emit('playtest')"
          >
            {{
              store.level === 'chapter'
                ? t('scriptEditor.editorHeader.playtestFromChapter')
                : t('scriptEditor.editorHeader.playtestFromStart')
            }}
          </button>
        </template>
      </span>
    </div>

    <!-- 试玩前置条件不满足时的常驻提示。等作者点了「试玩」才报，他会先对着
         一个卡住不动的画面困惑一阵 —— 那正是这条横幅要省掉的时间。 -->
    <div
      v-if="store.detail && store.readiness && !store.readiness.ok"
      class="flex
        items-center
        gap-2.5
        mx-5
        mb-2
        border
        border-amber-300/30
        rounded-lg
        px-3
        py-[7px]
        text-[0.76rem]
        leading-[1.7]
        text-amber-100/90
        bg-amber-300/10"
    >
      <span
        class="shrink-0
          border
          border-amber-300/40
          rounded-full
          px-2
          py-px
          text-[0.66rem]
          font-semibold
          text-amber-300"
        >{{ t('scriptEditor.scriptEditor.playtestBlocked') }}</span
      >
      <span>{{ store.readiness.reason }}</span>
    </div>
  </div>
</template>
