<template>
  <router-view />
  <!-- 将光标特效 teleport 到 body，避免 #app 上的 CSS zoom 导致坐标偏移 -->
  <Teleport to="body">
    <CursorEffects />
  </Teleport>

  <!-- 全局通知组件（直接从 uiStore 读取状态） -->
  <!-- 与桌宠专用通知组件区分开 -->
  <Notification v-if="route.path !== '/pet'" />
  <AchievementToast />
  <AdventureUnlockNotify />
  <AppDialog />
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
import { listSystemFonts, getImportedFonts, registerAllImportedFonts } from './api/services/font'

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
  width: 100vw;
  height: 100vh;
}
</style>
