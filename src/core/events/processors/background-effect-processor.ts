import type { IEventProcessor } from '../event-processor'
import type { ScriptBackgroundEffectEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { useSettingsStore } from '../../../stores/modules/settings'

// 限时特效（一闪）的序号守卫：只让"最后一次"限时特效的计时器负责还原，
// 避免连发两个限时特效时，先到的计时器把后到的特效提前清掉
let effectFlashSeq = 0

export default class BackgroundEffectProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'background_effect'
  }

  async processEvent(event: ScriptBackgroundEffectEvent): Promise<void> {
    const gameStore = useGameStore()
    const uiStore = useUIStore()

    // 处理对话逻辑
    gameStore.currentStatus = 'presenting'

    // BSOD 的剧本自带彩蛋文本（trace 行/独白）；切到非 BSOD 特效时清掉
    if (event.effect.split('+').includes('BSOD')) {
      uiStore.bsodText = event.text ?? ''
      uiStore.bsodEcho = event.echo ?? ''
    } else if (uiStore.bsodText || uiStore.bsodEcho) {
      uiStore.bsodText = ''
      uiStore.bsodEcho = ''
    }

    const duration = event.duration
    if (duration > 0) {
      // 限时特效（如 BloodUI 一闪）：展示 duration 秒后还原为之前的特效
      const previous = useSettingsStore().display.backgroundEffect
      const mySeq = ++effectFlashSeq
      uiStore.setBackgroundEffect(event.effect)
      setTimeout(() => {
        if (mySeq !== effectFlashSeq) return
        const current = useSettingsStore().display.backgroundEffect
        if (current === event.effect) {
          uiStore.setBackgroundEffect(previous || 'None')
        }
      }, duration * 1000)
      return
    }

    uiStore.setBackgroundEffect(event.effect)
  }
}
