/**
 * Ctrl+滚轮 全局 UI 缩放 Composable
 *
 * 按住 Ctrl 并滚动鼠标滚轮来缩放整个 UI 界面。
 * 使用 CSS zoom 属性在 #app 元素上实现均匀缩放（Chromium / WebView2 原生支持）。
 * 缩放级别保存在 localStorage 中，跨会话持久化。
 *
 * 在 App.vue 中调用一次以激活全局缩放功能。
 */

import { onUnmounted, ref } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'

const ZOOM_STORAGE_KEY = 'lingchat-ui-zoom'
const ZOOM_STEP = 0.05
const ZOOM_MIN = 0.5
const ZOOM_MAX = 2.0
const ZOOM_DEFAULT = 1.0
const ZOOM_DECIMALS = 2

/** 防抖 toast 的最小间隔（毫秒），避免滚轮时频繁弹出通知 */
const TOAST_DEBOUNCE_MS = 200

let lastToastTime = 0

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

/** 应用缩放到 DOM */
function applyZoom(level: number): void {
  const app = document.getElementById('app')
  if (app) {
    app.style.zoom = level.toString()
  }
}

/**
 * 激活 Ctrl+滚轮 UI 缩放功能。
 * 应在 App.vue 等始终挂载的根组件中调用一次。
 */
export function useZoom(): void {
  const currentZoom = ref<number>(loadZoom())

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
    currentZoom.value = Math.round(clamped * Math.pow(10, ZOOM_DECIMALS)) / Math.pow(10, ZOOM_DECIMALS)

    applyZoom(currentZoom.value)
    saveZoom(currentZoom.value)

    // 防抖显示缩放百分比 toast
    const now = Date.now()
    if (now - lastToastTime > TOAST_DEBOUNCE_MS) {
      lastToastTime = now
      const uiStore = useUIStore()
      uiStore.showInfo({
        title: '缩放',
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
