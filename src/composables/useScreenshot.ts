import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

// Shared screenshot state for pet mode (used by GameRoleAvatar + ChatInput)
const hasScreenshot = ref(false)
const screenshotBase64 = ref<string | null>(null)
const isCapturing = ref(false)
let unlisten: (() => void) | null = null
let unlistenCanceled: (() => void) | null = null
let initCount = 0
let pendingUnlisten: Promise<(() => void)> | null = null
let pendingUnlistenCanceled: Promise<(() => void)> | null = null

export function useScreenshot() {
  function init() {
    if (initCount++ > 0) return
    const p1 = listen<{ base64: string }>('screenshot:captured', (event) => {
      screenshotBase64.value = event.payload.base64
      hasScreenshot.value = true
      isCapturing.value = false
    }).then((fn) => {
      unlisten = fn
      pendingUnlisten = null
      return fn
    })
    pendingUnlisten = p1

    const p2 = listen('screenshot:cancelled', () => {
      hasScreenshot.value = false
      isCapturing.value = false
    }).then((fn) => {
      unlistenCanceled = fn
      pendingUnlistenCanceled = null
      return fn
    })
    pendingUnlistenCanceled = p2
  }

  function destroy() {
    if (--initCount > 0) return
    if (unlisten) {
      unlisten()
      unlisten = null
    } else if (pendingUnlisten) {
      pendingUnlisten.then((fn) => fn())
    }

    if (unlistenCanceled) {
      unlistenCanceled()
      unlistenCanceled = null
    } else if (pendingUnlistenCanceled) {
      pendingUnlistenCanceled.then((fn) => fn())
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
