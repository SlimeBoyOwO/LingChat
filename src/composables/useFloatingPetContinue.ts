/**
 * 单击悬浮桌宠 -> 推进聊天对话（spec 7.5）。
 *
 * 与 useFloatingPetBridge 解耦：本 composable 只挂 onPetEvent 单个 tap 订阅，
 * 不动 store、不做 invoke；适合放在 MainChat.vue 同时持有 gameDialogRef 的位置。
 *
 * 桌面 / iOS 上 tap 永远不会发出 (plugin 不会触发 AndroidService)，所以这里
 * 没有平台分支；该订阅在非 Android 设备上是空转。
 */
import { onBeforeUnmount, onMounted, type Ref } from 'vue'
import { onPetEvent } from '@/api/services/floating-pet'

type ContinueDialogFn = (isPlayerTrigger: boolean) => boolean
type DialogInstance = { continueDialog?: ContinueDialogFn } | null

export function useFloatingPetContinue(
  dialogRef: Ref<DialogInstance>,
) {
  let unbind: (() => void) | null = null

  onMounted(() => {
    unbind = onPetEvent((event) => {
      if (event.type !== 'tap') return
      const inst = dialogRef.value
      if (!inst || typeof inst.continueDialog !== 'function') return
      // 当 autoMode 也开着时,现有 manualTriggerContinue 内部会 cancel
      // 调度,这样 tap 也打断 autoAdvance 计时器,保持一致。
      inst.continueDialog(true)
    })
  })

  onBeforeUnmount(() => {
    unbind?.()
    unbind = null
  })
}
