<template>
  <router-view />
  <CursorEffects />

  <!-- 全局通知组件（直接从 uiStore 读取状态） -->
  <Notification />
  <AchievementToast />
  <AdventureUnlockNotify />
  <AppDialog />
  <UpdateDialog
    :visible="updatePhase !== 'idle' && updatePhase !== 'checking' || updatePhase === 'checking' && showUpdateDialog"
    :phase="updatePhase"
    :app-version="updateAppVersion"
    :app-release-notes="updateAppReleaseNotes"
    :data-info="updateDataInfo"
    :data-progress="updateDataProgress"
    :error-message="updateErrorMessage"
    @update="handleInstallUpdates"
    @later="handleRemindLater"
    @close="handleUpdateClose"
  />
</template>

<script setup>
import { onMounted, onUnmounted, ref } from 'vue'
import CursorEffects from './components/effects/CursorEffects.vue'
import Notification from './components/ui/Notification.vue'
import AchievementToast from './components/ui/AchievementToast.vue'
import AdventureUnlockNotify from './components/ui/AdventureUnlockNotify.vue'
import AppDialog from './components/ui/AppDialog.vue'
import UpdateDialog from './components/UpdateDialog.vue'
import { initUIStore } from './stores/modules/ui/ui'
import { useAchievementStore } from './stores/modules/ui/achievement'
import { useUpdater } from './composables/useUpdater'

// 在使用 <router-view> 的情况下，通常不需要在这里再导入具体的页面组件了

// ─── 更新检查 ────────────────────────────────────────────────

const updater = useUpdater()
const {
  phase: updatePhase,
  appVersion: updateAppVersion,
  appReleaseNotes: updateAppReleaseNotes,
  dataInfo: updateDataInfo,
  dataProgress: updateDataProgress,
  errorMessage: updateErrorMessage,
} = updater

const showUpdateDialog = ref(false)

async function checkUpdatesOnStartup() {
  // 延迟 3 秒后检查更新（不阻塞启动体验）
  setTimeout(async () => {
    try {
      const hasUpdate = await updater.checkForUpdates()
      if (hasUpdate) {
        showUpdateDialog.value = true
      }
    } catch {
      // 静默失败 — 不影响正常使用
    }
  }, 3000)
}

async function handleInstallUpdates() {
  try {
    await updater.installAllUpdates()
    // 如果有 app 更新，installAllUpdates 会调用 relaunch()
    // 如果没有 app 更新（仅 data 更新），phase 变为 complete
  } catch {
    // 错误已通过 phase 状态反映
  }
}

function handleRemindLater() {
  updater.remindLater()
  showUpdateDialog.value = false
}

function handleUpdateClose() {
  updater.reset()
  showUpdateDialog.value = false
}

// ─── 键盘处理 ────────────────────────────────────────────────

const handleKeyDown = (event) => {
  if (event.key === 'F11') {
    event.preventDefault()
    if (
      window.pywebview &&
      window.pywebview.api &&
      typeof window.pywebview.api.toggle_fullscreen === 'function'
    ) {
      // 调用从 Python 暴露的函数
      window.pywebview.api.toggle_fullscreen()
    } else {
      console.error('全屏API不可用。')
    }
  }
}

onMounted(() => {
  // 初始化 UI Store（加载角色 tips）
  initUIStore()

  // 供成就系统控制台测试用，在 window 对象中注册一些方法
  const achievementStore = useAchievementStore()
  window.requestAchievementUnlock = (data) => achievementStore.notifyBackendUnlock(data)
  window.showAchievement = (data) => achievementStore.addAchievement(data)
  // 成就系统启动WebSocket监听
  achievementStore.listenForUnlocks()

  // 初始化更新检查（延迟执行，不阻塞启动）
  updater.init()
  checkUpdatesOnStartup()

  // 等待 pywebview API 准备就绪
  window.addEventListener('pywebviewready', () => {
    window.addEventListener('keydown', handleKeyDown)
  })
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown)
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
  font-family:
    -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
  overflow: hidden;
  background: transparent;
  /* 确保body背景透明，不遮挡我们的背景图 */
}

#app {
  width: 100vw;
  height: 100vh;
}
</style>
