<template>
  <!-- Windows 静态快照背景：blur 在图片自身，不再用 backdrop-filter 持续消耗 GPU -->
  <div
    v-if="isWindowsMode && shouldShowOverlay"
    class="fixed inset-0 z-[999] overflow-hidden transition-opacity duration-300"
    :style="{ opacity: overlayOpacity }"
  >
    <img
      v-show="snapshotSrc && !snapshotFailed"
      ref="snapshotImgRef"
      :src="snapshotSrc ?? undefined"
      class="w-full h-full object-cover blur-[8px] brightness-[0.85] scale-[1.02] block transition-opacity duration-300"
      :class="imgReady ? 'opacity-100' : 'opacity-0'"
      draggable="false"
      alt=""
      @load="onSnapshotLoad"
      @error="onSnapshotError"
    />
    <div
      class="absolute inset-0 transition-colors duration-300"
      :class="snapshotFailed ? 'bg-black/72' : 'bg-black/35'"
    ></div>
  </div>
  <div
    v-else-if="shouldShowOverlay"
    class="fixed inset-0 bg-black/70 backdrop-blur-[8px] z-[999] transition-opacity duration-300"
    :style="{ opacity: overlayOpacity }"
  ></div>
  <div
    class="fixed inset-0 z-[1000] bg-transparent text-[var(--text-primary,#fff)] flex flex-col h-full"
    v-show="uiStore.showSettings"
  >
    <div class="shrink-0 w-full">
      <SettingsNav ref="settingsNavRef" @remove-more-menu-from-a="onAddFromA" />
    </div>

    <div
      class="w-full flex-1 relative overflow-hidden"
      ref="contentRef"
      @touchstart="onTouchStart"
      @touchend="onTouchEnd"
    >
      <Transition :name="transitionName">
        <!-- KeepAlive 缓存设置子页面实例：切换时只激活/停用，不销毁重建，保留状态 -->
        <KeepAlive>
          <component
            :is="currentTabComponent"
            :key="uiStore.currentSettingsTab"
            class="absolute inset-0 overflow-y-auto"
            ref="settingsAdvanceRef"
            @remove-more-menu-from-b="onAddFromB"
          />
        </KeepAlive>
      </Transition>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  SettingsText,
  SettingsSave,
  SettingsSound,
  SettingsHistory,
  SettingsAdvance,
  SettingsCharacter,
  SettingsBackground,
  SettingsAchievement,
  SettingsAdventure,
  SettingsLog,
  SettingsPlugins,
} from './pages'
import SettingsNav from './SettingsNav.vue'
import { useUIStore } from '../../stores/modules/ui/ui'
import { ref, watch, computed, nextTick, type Component } from 'vue'
import { isWindows } from '@/utils/platform'
import { useSettingsSnapshot } from '@/composables/useSettingsSnapshot'

const uiStore = useUIStore()
const { snapshotSrc, snapshotFailed } = useSettingsSnapshot()
const isWindowsMode = computed(() => isWindows())

// 快照图就绪再淡入：避免 file:// 解码未完成时的闪白/半帧
const imgReady = ref(false)
const snapshotImgRef = ref<HTMLImageElement | null>(null)

function onSnapshotLoad() {
  imgReady.value = true
}

function onSnapshotError() {
  imgReady.value = false
  // 图片解码失败则走兜底深色遮罩
  snapshotFailed.value = true
}

watch(snapshotSrc, (newVal) => {
  if (!newVal || snapshotFailed.value) {
    imgReady.value = false
    return
  }
  imgReady.value = false
  // 已缓存图片可能同步完成，nextTick 检查 complete 兜底
  nextTick(() => {
    const el = snapshotImgRef.value
    if (el && el.complete && el.naturalWidth > 0) {
      imgReady.value = true
    }
  })
})

watch(snapshotFailed, (failed) => {
  if (failed) imgReady.value = false
  else if (snapshotSrc.value) {
    // 从失败恢复且已有图时，重新检查就绪
    imgReady.value = false
    nextTick(() => {
      const el = snapshotImgRef.value
      if (el && el.complete && el.naturalWidth > 0) imgReady.value = true
    })
  }
})

// 获取 A 组件和 B 组件的 Ref 实例
const settingsNavRef = ref<InstanceType<typeof SettingsNav> | null>(null)
const settingsAdvanceRef = ref<InstanceType<typeof SettingsAdvance> | null>(null)

// 添加延迟状态（带 session 守卫，避免快速开关时旧定时器影响新会话）
const shouldShowOverlay = ref(false)
const overlayOpacity = ref(0)
let overlaySession = 0
let showTimer: ReturnType<typeof setTimeout> | null = null
let hideTimer: ReturnType<typeof setTimeout> | null = null

watch(
  () => uiStore.showSettings,
  (newVal) => {
    const mySession = ++overlaySession
    if (showTimer) {
      clearTimeout(showTimer)
      showTimer = null
    }
    if (hideTimer) {
      clearTimeout(hideTimer)
      hideTimer = null
    }
    if (newVal) {
      // 显示时：立即显示元素，然后延迟改变透明度
      shouldShowOverlay.value = true
      showTimer = setTimeout(() => {
        if (mySession !== overlaySession) return
        overlayOpacity.value = 1
      }, 10) // 使用很小的延迟确保浏览器有机会渲染
    } else {
      // 隐藏时：先改变透明度，然后延迟隐藏元素
      overlayOpacity.value = 0
      hideTimer = setTimeout(() => {
        if (mySession !== overlaySession) return
        shouldShowOverlay.value = false
      }, 300) // 与 CSS transition 0.3s 对齐，避免旧 hide 覆盖新 show
    }
  },
  { immediate: true },
)

// 关闭遮罩时重置就绪态，供下次打开复用（随外层 opacity 一起淡出，无需内层反向动画）
watch(
  () => shouldShowOverlay.value,
  (show) => {
    if (!show) imgReady.value = false
  },
)

// ========== 手机端左右滑动切换标签 ==========
// 导航栏在顶部，手机端通过水平滑动内容区切换设置页
// 标签顺序与 SettingsNav 导航一致
const TABS = [
  'character',
  'adventure',
  'text',
  'background',
  'sound',
  'history',
  'achievement',
  'save',
  'advance',
  'log',
  'plugins',
] as const

// 标签 → 组件映射（推入推出转场用 v-if 动态组件）
const tabComponents: Record<string, Component> = {
  save: SettingsSave,
  text: SettingsText,
  sound: SettingsSound,
  advance: SettingsAdvance,
  adventure: SettingsAdventure,
  history: SettingsHistory,
  achievement: SettingsAchievement,
  character: SettingsCharacter,
  background: SettingsBackground,
  log: SettingsLog,
  plugins: SettingsPlugins,
}
const currentTabComponent = computed(() => tabComponents[uiStore.currentSettingsTab])
// 转场方向：左滑下一项 → slide-left（新页从右进）；右滑上一项 → slide-right
const transitionName = ref<'slide-left' | 'slide-right'>('slide-left')

// 依据 TABS 顺序决定滑动方向：目标索引更大 → 下一项（slide-left）；更小 → 上一项（slide-right）。
// watch 默认 flush: 'pre'，在重渲染前更新 transitionName，Transition 开始动画时能取到正确方向。
watch(
  () => uiStore.currentSettingsTab,
  (newTab, oldTab) => {
    if (!oldTab || oldTab === newTab) return
    const newIdx = (TABS as readonly string[]).indexOf(newTab)
    const oldIdx = (TABS as readonly string[]).indexOf(oldTab)
    // 目标/来源不在滑动顺序里（理论不应发生）→ 保持原方向
    if (newIdx === -1 || oldIdx === -1) return
    transitionName.value = newIdx > oldIdx ? 'slide-left' : 'slide-right'
  },
)

const contentRef = ref<HTMLElement | null>(null)
let touchStartX = 0
let touchStartY = 0
let touchOnHorizontalScrollable = false
let isSwipeAnimating = false

// 判断触摸起点是否在"可滚动"容器内（纵向列表如存档/日志、横向表格等）。
// 是则不触发页面切换——用户可能在拖动内容或滚动条，不该切页。
// 只排除"确实有溢出可滚"的容器：内容不满的页面（无可滚动区域）仍可滑动切换。
function isInsideScrollable(el: Element | null): boolean {
  while (el && el !== contentRef.value) {
    // 数值调节滑块（原生 range / 自定义 Slider）→ 拖动它不该切页
    if (el.tagName === 'INPUT' && (el as HTMLInputElement).type === 'range') return true
    // 横向可滚动容器（如日志页横向表格）→ 拖动横向内容不该切页。
    // 竖向滚动容器不在此列：竖向滚动由 onTouchEnd 的 |dx| <= |dy| 判断兜底，
    // 竖向列表里做"明显横向"滑动仍可切页。
    const s = getComputedStyle(el)
    if (
      (s.overflowX === 'auto' || s.overflowX === 'scroll') &&
      el.scrollWidth > el.clientWidth + 4
    ) {
      return true
    }
    el = el.parentElement
  }
  return false
}

const onTouchStart = (e: TouchEvent) => {
  touchStartX = e.touches[0].clientX
  touchStartY = e.touches[0].clientY
  touchOnHorizontalScrollable = isInsideScrollable(e.target as Element)
}

const onTouchEnd = (e: TouchEvent) => {
  // 仅小屏（手机）生效
  if (!uiStore.isSmallScreen) return
  // 起点在可横向滚动区域（日志页等）→ 让原生滚动处理
  if (touchOnHorizontalScrollable) return
  if (isSwipeAnimating) return

  const dx = e.changedTouches[0].clientX - touchStartX
  const dy = e.changedTouches[0].clientY - touchStartY

  // 只响应明显的水平滑动（避免和垂直滚动/滑块冲突）
  if (Math.abs(dx) < 50 || Math.abs(dx) <= Math.abs(dy)) return

  const currentIdx = TABS.indexOf(uiStore.currentSettingsTab as (typeof TABS)[number])
  let nextIdx = dx < 0 ? currentIdx + 1 : currentIdx - 1 // 左滑 → 下一个，右滑 → 上一个
  if (nextIdx < 0) nextIdx = TABS.length - 1
  if (nextIdx >= TABS.length) nextIdx = 0

  isSwipeAnimating = true
  uiStore.setSettingsTab(TABS[nextIdx])
  setTimeout(() => {
    isSwipeAnimating = false
  }, 300)
}

// 提供给 SettingsAdvance 的对外暴露接口（示例）
const onAddFromA = () => {
  // A 组件转发的事件处理
}

const onAddFromB = () => {
  // B 组件转发的事件处理
}

// 在父组件中暴露引用（如需要）
defineExpose({
  settingsNavRef,
  settingsAdvanceRef,
})
</script>

<style scoped>
.slide-left-enter-active,
.slide-left-leave-active,
.slide-right-enter-active,
.slide-right-leave-active {
  /* 与剧本编辑器同一套动画参数：起步即快的缓动（而非默认的慢起步）——
     切 Tab 时新页挂载/旧页缓存 DOM 重插会有 1~2 帧主线程开销，慢起步缓动
     会让旧页看起来「卡在原地」形成残留；快速起步则把这点开销掩盖掉。 */
  transition: transform 0.32s cubic-bezier(0.32, 0.72, 0, 1);
}

.slide-left-enter-from {
  transform: translateX(100%);
}
.slide-left-leave-to {
  /* 旧页滑出多一点（±150%），保证整页宽度的 Tab 也彻底离开视口 */
  transform: translateX(-150%);
}

.slide-right-enter-from {
  transform: translateX(-100%);
}
.slide-right-leave-to {
  transform: translateX(150%);
}

.slide-left-enter-to,
.slide-left-leave-from,
.slide-right-enter-to,
.slide-right-leave-from {
  transform: translateX(0);
}
</style>