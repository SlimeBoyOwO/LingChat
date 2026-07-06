/**
 * 久坐喝水提醒 Composable
 *
 * 每 40 分钟通过 Tauri 系统通知提醒用户起身活动。
 * 通知图标使用 public/pictures/icons/icon.ico。
 *
 * 在 App.vue 中调用一次以激活全局定时器。
 */
import { watch, onUnmounted } from 'vue'
import {
  sendNotification,
  isPermissionGranted,
  requestPermission,
} from '@tauri-apps/plugin-notification'
import { useSettingsStore } from '@/stores/modules/settings'

const REMINDER_INTERVAL_MS = 40 * 60 * 1000 // 40 分钟
const NOTIFICATION_TITLE = 'LingChat 久坐提醒'
const NOTIFICATION_BODY = '久坐时间有点长，记得活动一下哦'
const APP_ICON_PATH = '/pictures/icons/icon.ico'
const AUDIO_PATH = '/audio_effects/生气.wav'

let timerId: ReturnType<typeof setInterval> | null = null

/** 播放提醒音效 */
function playReminderSound(): void {
  try {
    const audio = new Audio(AUDIO_PATH)
    audio.volume = 0.8
    audio.play().catch(() => {
      // 静默忽略自动播放策略限制
    })
  } catch {
    // 静默忽略音频播放失败
  }
}

/** 发送久坐提醒系统通知 */
async function showReminderNotification(): Promise<void> {
  try {
    let granted = await isPermissionGranted()
    if (!granted) {
      const result = await requestPermission()
      granted = result === 'granted'
    }

    if (!granted) {
      console.warn('[SedentaryReminder] 通知权限未授予，跳过本次提醒')
      return
    }

    playReminderSound()

    await sendNotification({
      title: NOTIFICATION_TITLE,
      body: NOTIFICATION_BODY,
      icon: APP_ICON_PATH,
    })
  } catch (e) {
    console.error('[SedentaryReminder] 发送通知失败:', e)
  }
}

/** 启动定时器（先停止已有的，避免重复） */
function startTimer(): void {
  stopTimer()
  timerId = setInterval(showReminderNotification, REMINDER_INTERVAL_MS)
  console.log('[SedentaryReminder] 定时器已启动（间隔 40 分钟）')
}

/** 停止定时器 */
function stopTimer(): void {
  if (timerId !== null) {
    clearInterval(timerId)
    timerId = null
    console.log('[SedentaryReminder] 定时器已停止')
  }
}

/**
 * 激活久坐提醒功能。
 * 监听设置开关，自动启动/停止定时器。
 * 应在 App.vue 等始终挂载的组件中调用。
 */
export function useSedentaryReminder(): void {
  const settingsStore = useSettingsStore()

  watch(
    () => settingsStore.text.sedentaryReminder,
    (enabled) => {
      if (enabled) {
        startTimer()
      } else {
        stopTimer()
      }
    },
    { immediate: true },
  )

  onUnmounted(() => {
    stopTimer()
  })
}
