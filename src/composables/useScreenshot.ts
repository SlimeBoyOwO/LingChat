import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

// Shared screenshot state for pet mode (used by GameRoleAvatar + ChatInput)
const hasScreenshot = ref(false)
const screenshotBase64 = ref<string | null>(null)
const isCapturing = ref(false)
let unlisten: (() => void) | null = null
let initCount = 0

export function useScreenshot() {
  function init() {
    if (initCount++ > 0) return
    listen<{ base64: string }>('screenshot:captured', (event) => {
      screenshotBase64.value = event.payload.base64
      hasScreenshot.value = true
      isCapturing.value = false
    }).then((fn) => {
      unlisten = fn
    })
  }

  function destroy() {
    if (--initCount > 0) return
    if (unlisten) {
      unlisten()
      unlisten = null
    }
  }

  async function start() {
    if (isCapturing.value) return
    isCapturing.value = true
    try {
      await invoke('start_screenshot')
    } catch (error) {
      console.error('启动截图失败:', error)
      isCapturing.value = false
    }
  }

  function clear() {
    if (hasScreenshot.value) {
      hasScreenshot.value = false
      screenshotBase64.value = null
    }
  }

  return {
    hasScreenshot,
    screenshotBase64,
    isCapturing,
    init,
    destroy,
    start,
    clear,
  }
}
