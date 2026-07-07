import { ref, watch, onMounted } from 'vue'
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
const _inputListeners = new Set<() => void>()

/** 各输入组件在 watch 中调用此函数来更新输入状态 */
export function setInputHasText(val: boolean) {
  if (_inputHasText === val) return
  _inputHasText = val
  _inputListeners.forEach((fn) => fn())
}

export function useCanDeliver() {
  const router = useRouter()
  const uiStore = useUIStore()

  const canDeliver = ref(false)
  let lastInvoked: boolean | null = null

  function recompute() {
    const onChatRoute = CHAT_ROUTES.includes(router.currentRoute.value.name as string)
    canDeliver.value = onChatRoute && !uiStore.showSettings && !_inputHasText
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

  // 值翻转时通知后端
  watch(canDeliver, (val) => {
    if (val !== lastInvoked) {
      lastInvoked = val
      invoke('proactive_set_can_deliver', { canDeliver: val })
        .catch((e) => console.error('[CanDeliver] invoke failed:', e))
    }
  })

  return { canDeliver }
}
