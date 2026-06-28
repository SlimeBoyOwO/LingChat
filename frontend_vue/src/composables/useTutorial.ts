/**
 * useTutorialStep composable
 *
 * 组件可以调用此 composable 来响应教程进入指定步骤的事件。
 * 当教程进入与 stepId 匹配的步骤时，callback 被触发。
 *
 * @example
 * ```ts
 * useTutorialStep('llm-config', () => {
 *   // 当教程进入 llm-config 步骤时，执行一些额外操作
 * })
 * ```
 */
import { watch } from 'vue'
import { useTutorialStore } from '../stores/modules/tutorial'

/** 教程步骤回调函数 */
type TutorialStepCallback = () => void

/**
 * 监听教程进入指定步骤
 * @param stepId - 步骤 ID
 * @param callback - 进入步骤时触发的回调
 */
export function useTutorialStep(stepId: string, callback: TutorialStepCallback) {
  const tutorialStore = useTutorialStore()

  watch(
    () => tutorialStore.currentStep?.id,
    (newId) => {
      if (newId === stepId) {
        callback()
      }
    },
    { immediate: true },
  )
}
