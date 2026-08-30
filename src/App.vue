<template>
  <router-view />
  <!-- 将光标特效 teleport 到 body，避免 #app 上的整体缩放（transform: scale）导致坐标偏移 -->
  <Teleport to="body">
    <CursorEffects />
  </Teleport>

  <!-- 全局通知组件（直接从 uiStore 读取状态） -->
  <!-- 与桌宠专用通知组件区分开 -->
  <!-- 弹窗类组件仅主窗口挂载：日志等独立窗口复用 App.vue，不重复弹出 -->
  <Notification v-if="isMainWindow && route.path !== '/pet'" />
  <AchievementToast v-if="isMainWindow" />
  <AdventureUnlockNotify v-if="isMainWindow" />
  <AppDialog v-if="isMainWindow" />
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, watch } from 'vue'
import { useRoute } from 'vue-router'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import CursorEffects from './components/effects/CursorEffects.vue'
import Notification from './components/ui/Notification.vue'
import AchievementToast from './components/ui/AchievementToast.vue'
import AdventureUnlockNotify from './components/ui/AdventureUnlockNotify.vue'
import AppDialog from './components/ui/AppDialog.vue'
import { initUIStore } from './stores/modules/ui/ui'
import { i18n } from './locales'
import { useSettingsStore } from './stores/modules/settings'
import { useLlmProvidersStore } from './stores/modules/llm-providers'
import { useAchievementStore } from './stores/modules/ui/achievement'
import { useDialogStore } from './stores/modules/ui/dialog'
import { useSedentaryReminder } from './composables/useSedentaryReminder'
import { useUpdater } from './composables/useUpdater'
import { useCanDeliver } from './composables/useCanDeliver'
import { useZoom } from './composables/useZoom'
import { useAsrInput } from './composables/useAsrInput'
import { listSystemFonts, getImportedFonts, registerAllImportedFonts } from './api/services/font'
import { isMobile } from './utils/platform'

// ─── 激活主动对话投放条件上报（仅在此处挂载一次） ────────────
useCanDeliver()

// 激活 Ctrl+滚轮 UI 全局缩放
useZoom()

// ─── 久坐提醒 ────────────────────────────────────────────────
useSedentaryReminder()

// ─── 全局字体 ────────────────────────────────────────────────
// 把设置中的自定义字体名同步到 <html> 的 --font-app；
// 为空时 base.css 中的回退栈 --font-sans 生效。初始菜单 / 加载页因自带
// 显式 font-family 不会继承此变量，自动保持原有字体。
const settingsStore = useSettingsStore()
function applyFont(font?: string) {
  // 留空 → 软件默认（base.css 的 --font-sans 原版字体栈）
  document.documentElement.style.setProperty('--font-app', font ? `'${font}'` : '')
}
watch(() => settingsStore.text.fontFamily, applyFont, { immediate: true })

// 提前预取系统字体列表：在应用初始化时即调用一次 Rust 枚举并入内存缓存，
// 避免打开设置页时才触发 IPC 造成可感知的卡顿。注：忽略结果即可，
// SettingsText 进入时直接命中 font.ts 的缓存。
void listSystemFonts()

// 启动时加载导入字体并注册 @font-face 规则，确保用户之前导入的字
// 体在 settings store 恢复字体选择前已可用。
void getImportedFonts().then((fonts) => {
  registerAllImportedFonts(fonts)
})

// ─── 键盘处理 ────────────────────────────────────────────────

const route = useRoute()

// ─── 移动端键盘适配（Android / iOS）─────────────────────────
// 键盘弹出时把可见高度并入 --safe-area-inset-bottom（存在 .pb-safe/pb-safe-gap、
// 对话框 padding、MusicPlayer 等 var() 用法，UI 自动上移让位）。
// 仅移动端挂载：桌面端 visualViewport == window，这套逻辑是无操作死代码，不挂载。
const vv = window.visualViewport
// 基准底部安全区（无键盘时的 env 值，首个值即基线）
let safeBaseBottom = 0
let safeBaseInitialized = false
// 当前已施加的抬升量（几何解算用自然位置 = 当前底部 + 已抬升量，避免自引用震荡）
let currentLift = 0
// 键盘状态兜底轮询：外部/配件键盘等场景 vv 事件偶发不触发，
// 轮询 vv.height 变化触发重算（800ms 一次，开销可忽略）
let kbGuardTimer: ReturnType<typeof setInterval> | null = null
let lastKbSig = 0

const lockScroll = () => {
  window.scrollTo(0, 0)
  if (document.documentElement.scrollTop) document.documentElement.scrollTop = 0
  if (document.body.scrollTop) document.body.scrollTop = 0
}

// 页面级平移拦截（iOS 键盘收起后剩余的可滚动区）：根级 touchmove 直接 preventDefault，
// 内部滚动容器（聊天记录/设置页等 overflow-* / custom-scroll）不受影响
const preventRootTouchScroll = (e: TouchEvent) => {
  const t = e.target as HTMLElement | null
  if (
    t &&
    t.closest(
      '.overflow-y-auto, .overflow-x-auto, .overflow-auto, .overflow-y-scroll, .overflow-x-scroll, .overflow-scroll, .custom-scroll, .scrollbar-thin, [data-scrollable]',
    )
  ) {
    return
  }
  e.preventDefault()
}

// 移动端禁用 WebView 原生缩放（双指捏合 / 双击放大）：
// index.html 的 viewport meta（user-scalable=no + maximum-scale=1）与 base.css 的
// touch-action: manipulation 为主，这里再兜底拦截 WebKit 缩放手势与多触点手势，
// 确保 Android WebView / iOS WKWebView 都无法捏合或双击放大界面。
const preventZoomGestures = (e: Event) => {
  // iOS Safari/WKWebView 的捏合缩放会触发 gesturestart/change/end，preventDefault 可取消缩放
  if (e.type === 'gesturestart' || e.type === 'gesturechange' || e.type === 'gestureend') {
    e.preventDefault()
    return
  }
  // 触点 >= 2 即是双指捏合手势（Android/通用），preventDefault 取消缩放
  const te = e as TouchEvent
  if (te.touches && te.touches.length > 1) {
    e.preventDefault()
  }
}

const syncVisualViewport = () => {
  if (!vv || !isMobile()) return
  const root = document.documentElement
  if (!safeBaseInitialized) {
    // 初始同步：读取当前 env() 解析值作为基线（iOS: 34px 左右；桌面 0）
    const cur = parseFloat(getComputedStyle(root).getPropertyValue('--safe-area-inset-bottom')) || 0
    safeBaseBottom = Number.isFinite(cur) ? cur : 0
    safeBaseInitialized = true
  }

  // iPad 检测：现代 iPadOS 为桌面版 UA（MacIntel + 多点触控）
  const isIPad =
    navigator.userAgent.includes('iPad') ||
    (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1)

  // 键盘可见高度 = visualViewport 高度差（iOS 软键盘/配件条收缩量；
  // env(keyboard-inset-height) 无法通过 getComputedStyle 读取，只信 vv 差值）
  const kbd = window.innerHeight - vv.height

  // 键盘可见区顶部（可见区高度 = min(vv.height, 窗口 - 键盘高度)）
  const visibleHeight = Math.min(vv.height, window.innerHeight - kbd)

  // 不让位的情形：无键盘；或 iPad 悬浮小键盘（可拖动，挡到输入框用户会自行移开）
  if (kbd <= 20 || (isIPad && kbd < 200)) {
    currentLift = 0
  } else {
    // 几何解算让位：以「聚焦输入框自然底部 + 间距」相对键盘上方可见区的高度差为准。
    // 自然位置 = 当前底部 + 已施加抬升量（移除抬升影响），公式稳定不震荡。
    const ae = document.activeElement as HTMLElement | null
    if (ae && typeof ae.getBoundingClientRect === 'function') {
      const r = ae.getBoundingClientRect()
      const naturalBottom = r.bottom + currentLift
      if (naturalBottom - 16 > visibleHeight) {
        currentLift = Math.max(0, Math.round(naturalBottom - 16 - visibleHeight))
      }
    }
  }
  // 仿 Android：底部安全区 = 基线 + 抬升量
  root.style.setProperty('--safe-area-inset-bottom', `${safeBaseBottom + currentLift}px`)

  // 页面始终锚定原点（键盘弹出的系统 focus-scroll 与手势滚动都会被锁回）
  lockScroll()
}

// 旋转/分屏后安全区基线失效：iPhone 竖屏底部 inset ≈34px、横屏 ≈21px（灵动岛移到左右），
// iPad 台前调度改窗口尺寸同理。挂载时采样的基线在旋转后是旧值，这里强制重采样：
//   1. 先清掉本模块写入的内联覆盖（iOS 回落 :root 的 env() 实时解析新值；
//      Android 的 --safe-area-inset-* 由 MainActivity insets 监听注入，旋转后监听
//      会重新触发注入，此处的临时清空无影响）
//   2. 重置基线标记，下一次 sync 重读 env() 解析值作为新基线
const handleOrientationChange = () => {
  document.documentElement.style.removeProperty('--safe-area-inset-bottom')
  safeBaseInitialized = false
  currentLift = 0
  syncVisualViewport()
}

// 仅主窗口挂载全局弹窗（通知/成就/对话确认），日志窗口等复用 App.vue 的窗口不弹
const isMainWindow = getCurrentWindow().label === 'main'

// ASR 全局初始化（仅主窗口一次）：auto_listen 能量监测门控 + 事件监听。
// useAsrInput 状态是模块级单例，GameDialog / ChatInput（桌宠）的 mic 按钮
// 与这里共享同一会话。
if (isMainWindow) {
  useAsrInput()
}

const handleKeyDown = async (event: KeyboardEvent) => {
  if (event.key === 'F11') {
    event.preventDefault()

    // Pet 路由时不允许全屏
    if (route.path === '/pet') {
      return
    }

    try {
      const appWindow = getCurrentWindow()
      const isFullscreen = await appWindow.isFullscreen()
      await appWindow.setFullscreen(!isFullscreen)
    } catch (e) {
      console.error('全屏切换失败:', e)
    }
  }
}

// ─── 关闭确认 ────────────────────────────────────────────────

const dialogStore = useDialogStore()
let saveCompleted = false
let userConfirmedExit = false
let unlistenCloseReady: (() => void) | null = null
let unlistenCloseRequested: (() => void) | null = null

// 处理退出：两个条件都满足时调用 Rust exit_app
function tryExit() {
  if (saveCompleted && userConfirmedExit) {
    invoke('exit_app')
  }
}

onMounted(async () => {
  // 初始化 UI Store（加载角色 tips）
  initUIStore()

  // 启动时自动弹出独立日志窗口（仅主窗口触发，开关在日志页设置）
  if (
    getCurrentWindow().label === 'main' &&
    localStorage.getItem('lingchat_log_window_auto_open') === '1'
  ) {
    invoke('open_log_window').catch((e) => console.error('自动打开日志窗口失败:', e))
  }

  // 预加载 LLM 提供商配置，避免主界面因 store 未加载而误判未选择模型
  const llmStore = useLlmProvidersStore()
  llmStore.load().catch((e) => console.error('加载 LLM 提供商失败:', e))

  // 供成就系统控制台测试用，在 window 对象中注册一些方法
  const achievementStore = useAchievementStore()
  ;(window as any).requestAchievementUnlock = (data: any) =>
    achievementStore.notifyBackendUnlock(data)
  ;(window as any).showAchievement = (data: any) => achievementStore.addAchievement(data)
  // 成就系统启动WebSocket监听
  achievementStore.listenForUnlocks()

  // 注册 F11 全屏快捷键
  window.addEventListener('keydown', handleKeyDown)

  // ─── 移动端键盘：视觉视口收缩时同步布局视口 ──────────────────
  // WKWebView 固定布局下聚焦输入框弹出键盘时，布局视口不会自动收缩，
  // 页面比可视区高 → 整个 webview 可上下滑动、输入框被键盘盖住。
  // 这里跟随 visualViewport（键盘弹出=可视区高度），配合 index.html 的
  // interactive-widget=resizes-content 双保险。
  // 仅移动端挂载：桌面 visualViewport == window，此逻辑是无操作死代码。
  if (isMobile()) {
    if (vv) {
      vv.addEventListener('resize', syncVisualViewport)
      vv.addEventListener('scroll', syncVisualViewport)
    }
    window.addEventListener('orientationchange', handleOrientationChange)
    // 硬锁滚动：任何滚动（键盘 focus-scroll / 手势）立即归零
    window.addEventListener('scroll', lockScroll, { passive: true, capture: true })
    document.addEventListener('touchmove', preventRootTouchScroll, { passive: false })
    // 兜底：禁用双指/双击/捏合的原生缩放
    document.addEventListener('gesturestart', preventZoomGestures, { passive: false })
    document.addEventListener('gesturechange', preventZoomGestures, { passive: false })
    document.addEventListener('gestureend', preventZoomGestures, { passive: false })
    document.addEventListener('touchstart', preventZoomGestures, { passive: false })
    document.addEventListener('touchmove', preventZoomGestures, { passive: false })
    // 聚焦变化 → 重算让位（focusin 先清 0 由 vv resize 收敛，focusout 归零）
    document.addEventListener('focusin', syncVisualViewport, true)
    document.addEventListener('focusout', syncVisualViewport, true)
    if (vv) {
      // 兜底轮询：外部/配件键盘等场景 vv 事件偶发不触发，轮询高度变化重算
      kbGuardTimer = setInterval(() => {
        const sig = Math.round(vv.height)
        if (sig !== lastKbSig) {
          lastKbSig = sig
          syncVisualViewport()
        }
      }, 800)
    }
    lockScroll()
  }

  // ─── 关闭确认逻辑 ──────────────────────────────────────────

  // 1. 监听 Rust 存档完成事件
  unlistenCloseReady = await listen('app:close-ready', () => {
    saveCompleted = true
    tryExit()
  })

  // 2. 拦截窗口关闭请求（仅主窗口需要确认，其他窗口正常关闭）
  unlistenCloseRequested = await getCurrentWindow().onCloseRequested(
    async (event: { preventDefault: () => void }) => {
      if (getCurrentWindow().label !== 'main') return

      event.preventDefault()

      // 重置状态
      saveCompleted = false
      userConfirmedExit = false

      if (route.path === '/chat') {
        const confirmed = await dialogStore.confirm(
          i18n.global.t('common.exitMessage'),
          i18n.global.t('common.exitTitle'),
        )
        if (!confirmed) return // 用户取消，窗口保持打开
      }

      userConfirmedExit = true
      tryExit()
    },
  )
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown)
  if (isMobile()) {
    window.removeEventListener('orientationchange', handleOrientationChange)
    window.removeEventListener('scroll', lockScroll, { capture: true } as any)
    document.removeEventListener('touchmove', preventRootTouchScroll)
    document.removeEventListener('gesturestart', preventZoomGestures)
    document.removeEventListener('gesturechange', preventZoomGestures)
    document.removeEventListener('gestureend', preventZoomGestures)
    document.removeEventListener('touchstart', preventZoomGestures)
    document.removeEventListener('touchmove', preventZoomGestures)
    document.removeEventListener('focusin', syncVisualViewport, true)
    document.removeEventListener('focusout', syncVisualViewport, true)
    if (kbGuardTimer) {
      clearInterval(kbGuardTimer)
      kbGuardTimer = null
    }
    if (vv) {
      vv.removeEventListener('resize', syncVisualViewport)
      vv.removeEventListener('scroll', syncVisualViewport)
    }
  }
  if (unlistenCloseReady) unlistenCloseReady()
  if (unlistenCloseRequested) unlistenCloseRequested()
})
</script>

<style>
:root {
  /*全局变量*/
  --accent-color: #79d9ff;
  --menu-max-width: 1100px;
  --menu-max-width-half: 550px;
  /* 一个生动的天蓝色，可以根据你的品牌调整 */
}

/* 全局样式和字体 */
body,
html {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: transparent;
}

#app {
  /* 视口口径统一为动态视口（dvw/dvh，iOS 全屏态下 dvw 横屏自动排除左右安全区、dvh 竖屏含上下安全区）：
     #app 铺满整个视觉视口（含状态栏/Home 指示器区域），使各屏壁纸全出血显示；
     安全区内缩由各边缘元素通过 env(safe-area-inset-*)（桌面/Android 桌面为 0px，零回归）自行处理——
     已在全局提供 --safe-area-inset-* 变量与 .pt-safe/.pb-safe 工具类（见 base.css）。 */
  position: fixed;
  top: 0;
  left: 0;
  width: 100dvw;
  height: 100dvh;
}
</style>
