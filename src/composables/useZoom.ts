/**
 * Ctrl+滚轮 全局 UI 缩放 Composable
 *
 * 按住 Ctrl 并滚动鼠标滚轮来缩放整个 UI 界面。
 * 使用 `transform: scale` + 尺寸补偿在 #app 元素上实现均匀缩放：
 * #app 布局尺寸设为 `calc(100dvw / z) × calc(100dvh / z)` 再整体放大 z 倍，
 * 视觉上恒精确填满窗口 —— 旧实现用 CSS zoom 直接缩放 100vw×100vh，
 * 放大时内容溢出视口被裁切（像放大镜）、缩小时填不满窗口留下空白。
 * 缩放级别保存在 localStorage 中，跨会话持久化。
 *
 * 在 App.vue 中调用一次以激活全局缩放功能。
 */

import { onUnmounted, ref } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'
import { i18n } from '@/locales'

const ZOOM_STORAGE_KEY = 'lingchat-ui-zoom'
const ZOOM_STEP = 0.05
const ZOOM_MIN = 0.5
const ZOOM_MAX = 2.0
const ZOOM_DEFAULT = 1.0
const ZOOM_DECIMALS = 2

/** 防抖 toast 的最小间隔（毫秒），避免滚轮时频繁弹出通知 */
const TOAST_DEBOUNCE_MS = 200

let lastToastTime = 0

/**
 * 当前缩放级别（模块级共享 ref）。
 *
 * 放在 composable 外是为了让其他模块（如 EditorHeader 的 Tab 指示条）能读取
 * 当前缩放值做坐标换算 —— transform 方案下 getBoundingClientRect 返回视觉坐标
 * 而 scrollLeft 等是布局坐标，混用必须除以缩放值。
 */
export const currentZoom = ref<number>(loadZoom())

/** 读取持久化的缩放值 */
function loadZoom(): number {
  try {
    const stored = localStorage.getItem(ZOOM_STORAGE_KEY)
    if (stored) {
      const value = parseFloat(stored)
      if (!isNaN(value) && value >= ZOOM_MIN && value <= ZOOM_MAX) {
        return Math.round(value * 100) / 100
      }
    }
  } catch {
    // localStorage 不可用时静默回退
  }
  return ZOOM_DEFAULT
}

/** 持久化缩放值 */
function saveZoom(level: number): void {
  try {
    localStorage.setItem(ZOOM_STORAGE_KEY, level.toString())
  } catch {
    // localStorage 不可用时静默忽略
  }
}

/** 缩放值转百分比字符串 */
function toPercent(level: number): string {
  return `${Math.round(level * 100)}%`
}

/**
 * 应用缩放到 DOM。
 *
 * transform: scale + 尺寸补偿：transform 使 #app 成为 fixed 子孙的 containing
 * block，布局尺寸 = 视口 / z，缩放后视觉恒精确填满窗口（zoom 方案在 z>1 裁切、
 * z<1 留白）。副作用：fixed inset-0 元素相对 #app 盒定位，视觉仍填满视口。
 */
function applyZoom(level: number): void {
  const app = document.getElementById('app')
  if (app) {
    app.style.transformOrigin = 'top left'
    app.style.transform = `scale(${level})`
    // #app 铺满整个动态视口（含安全区）；同 App.vue 的 position: fixed 口径：
    // 布局尺寸 = 视口 / z，缩放后视觉恒精确填满视口。
    app.style.width = `calc(100dvw / ${level})`
    app.style.height = `calc(100dvh / ${level})`
  }
  // 防御性归零页面级滚动：transform 方案下 #app 布局尺寸 = 视口 / z，超出视口
  // 使 body 成为可滚动容器，任何 scrollIntoView/编程滚动都可能把 fixed 面板
  // （containing block 是 #app）滚出视口。归零保证缩放后页面始终锚定原点。
  // （源头修复见 SettingsNav：tab 指示条滚动已限定在 nav 容器内。）
  window.scrollTo(0, 0)
  document.body.scrollLeft = 0
  document.body.scrollTop = 0
}

/**
 * 激活 Ctrl+滚轮 UI 缩放功能。
 * 应在 App.vue 等始终挂载的根组件中调用一次。
 */
export function useZoom(): void {
  // 初始化时应用已保存的缩放
  applyZoom(currentZoom.value)

  const handleWheel = (event: WheelEvent) => {
    if (!event.ctrlKey) return

    // 阻止浏览器默认的缩放行为
    event.preventDefault()

    // 向下滚动（deltaY > 0）= 缩小，向上滚动（deltaY < 0）= 放大
    const delta = event.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP
    const newZoom = Math.round((currentZoom.value + delta) * 100) / 100
    const clamped = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, newZoom))

    // 四舍五入到指定小数位数
    currentZoom.value =
      Math.round(clamped * Math.pow(10, ZOOM_DECIMALS)) / Math.pow(10, ZOOM_DECIMALS)

    applyZoom(currentZoom.value)
    saveZoom(currentZoom.value)

    // 防抖显示缩放百分比 toast
    const now = Date.now()
    if (now - lastToastTime > TOAST_DEBOUNCE_MS) {
      lastToastTime = now
      const uiStore = useUIStore()
      uiStore.showInfo({
        title: i18n.global.t('stores.zoom.toastTitle'),
        message: toPercent(currentZoom.value),
        duration: 800,
      })
    }
  }

  // 使用 passive: false 以允许 preventDefault 阻止浏览器默认缩放
  window.addEventListener('wheel', handleWheel, { passive: false })

  onUnmounted(() => {
    window.removeEventListener('wheel', handleWheel)
  })
}
