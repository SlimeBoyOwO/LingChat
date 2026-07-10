import { onBeforeUnmount, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useUIStore } from '@/stores/modules/ui/ui'

/**
 * 前端上报的“当前是否适合投放主动对话”。
 * 条件：用户在聊天界面 且 设置面板未打开 且 输入框为空。
 *
 * 仅在最终布尔值翻转时调用 invoke（不会反复上报）。
 */

// ===== 可投放的路由名（聊天主界面 + 桌宠） =====
const CHAT_ROUTES = ['LingChat', 'PetMode']

// ===== 全局输入状态（由 GameDialog / ChatInput 组件上报） =====
let _inputHasText = false
let _externalWindowBlocked = false
const _inputListeners = new Set<() => void>()

/** 各输入组件在 watch 中调用此函数来更新输入状态 */
export function setInputHasText(val: boolean) {
  if (_inputHasText === val) return
  _inputHasText = val
  _inputListeners.forEach((fn) => fn())
}

/** 独立设置/覆盖层窗口打开时，由主窗口显式阻止主动回复。 */
export function setExternalDeliveryBlocked(val: boolean) {
  if (_externalWindowBlocked === val) return
  _externalWindowBlocked = val
  _inputListeners.forEach((fn) => fn())
}

export function useCanDeliver() {
  const router = useRouter()
  const uiStore = useUIStore()

  const canDeliver = ref(false)
  let acknowledged: boolean | null = null
  let syncing = false
  let retryTimer: number | null = null
  let retryDelayMs = 250
  let disposed = false

  function recompute() {
    const onChatRoute = CHAT_ROUTES.includes(router.currentRoute.value.name as string)
    canDeliver.value =
      onChatRoute && !uiStore.showSettings && !_inputHasText && !_externalWindowBlocked
  }

  // 监听变更
  watch(
    () => router.currentRoute.value.name,
    recompute,
    { immediate: true },
  )
  watch(
    () => uiStore.showSettings,
    recompute,
  )

  // 输入状态变化时重新计算
  _inputListeners.add(recompute)
  onBeforeUnmount(() => {
    disposed = true
    _inputListeners.delete(recompute)
    if (retryTimer !== null) window.clearTimeout(retryTimer)
  })

  // 串行同步，避免 true/false 两次 invoke 乱序完成后把后端留在过期状态。
  async function syncCanDeliver() {
    if (syncing || disposed) return
    syncing = true

    try {
      while (!disposed && acknowledged !== canDeliver.value) {
        const target = canDeliver.value
        try {
          await invoke('proactive_set_can_deliver', { canDeliver: target })
          acknowledged = target
          retryDelayMs = 250
          if (retryTimer !== null) {
            window.clearTimeout(retryTimer)
            retryTimer = null
          }
        } catch (error) {
          console.error('[CanDeliver] invoke failed:', error)
          retryTimer = window.setTimeout(() => {
            retryTimer = null
            void syncCanDeliver()
          }, retryDelayMs)
          retryDelayMs = Math.min(retryDelayMs * 2, 5000)
          break
        }
      }
    } finally {
      syncing = false
    }
  }

  // 值翻转时通知后端；只有调用成功后才更新 acknowledged。
  watch(
    canDeliver,
    () => {
      void syncCanDeliver()
    },
    { immediate: true },
  )

  return { canDeliver }
}
