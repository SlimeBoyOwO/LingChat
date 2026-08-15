<template>
  <div class="blur-overlay" v-if="shouldShowOverlay" :style="{ opacity: overlayOpacity }"></div>
  <div class="settings-panel flex flex-col h-full" v-show="uiStore.showSettings">
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
import { ref, watch, computed, type Component } from 'vue'
import { isAndroid } from '@/utils/platform'

const uiStore = useUIStore()

// 获取 A 组件和 B 组件的 Ref 实例
const settingsNavRef = ref<InstanceType<typeof SettingsNav> | null>(null)
const settingsAdvanceRef = ref<InstanceType<typeof SettingsAdvance> | null>(null)

// 添加延迟状态
const shouldShowOverlay = ref(false)
const overlayOpacity = ref(0)

watch(
  () => uiStore.showSettings,
  (newVal) => {
    if (newVal) {
      // 显示时：立即显示元素，然后延迟改变透明度
      shouldShowOverlay.value = true
      setTimeout(() => {
        overlayOpacity.value = 1
      }, 10) // 使用很小的延迟确保浏览器有机会渲染
    } else {
      // 隐藏时：先改变透明度，然后延迟隐藏元素
      overlayOpacity.value = 0
      setTimeout(() => {
        shouldShowOverlay.value = false
      }, 100) // 匹配你的动画持续时间
    }
  },
  { immediate: true },
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
  // 插件系统由 RustPython 驱动，移动端不编译（cfg(desktop)），Android 上不显示该 tab
  ...(isAndroid() ? [] : ['plugins']),
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
.blur-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.7);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  z-index: 999;
  transition: opacity 0.3s ease;
  opacity: 0;
}

.settings-panel {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 1000;
  background: transparent;
  color: var(--text-primary, #fff);
}

.slide-left-enter-active,
.slide-left-leave-active,
.slide-right-enter-active,
.slide-right-leave-active {
  transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.slide-left-enter-from {
  transform: translateX(100%);
}
.slide-left-leave-to {
  transform: translateX(-100%);
}

.slide-right-enter-from {
  transform: translateX(-100%);
}
.slide-right-leave-to {
  transform: translateX(100%);
}

.slide-left-enter-to,
.slide-left-leave-from,
.slide-right-enter-to,
.slide-right-leave-from {
  transform: translateX(0);
}
</style>