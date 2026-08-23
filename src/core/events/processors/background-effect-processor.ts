import type { IEventProcessor } from '../event-processor'
import type { ScriptBackgroundEffectEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { useSettingsStore } from '../../../stores/modules/settings'

// Only the newest effect may restore/reset the shared layer.
let effectFlashSeq = 0
let activeTimer: number | null = null
let detachAbort: (() => void) | null = null

function detachCurrentAbort() {
  detachAbort?.()
  detachAbort = null
}

export default class BackgroundEffectProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'background_effect'
  }

  async processEvent(event: ScriptBackgroundEffectEvent, signal?: AbortSignal): Promise<void> {
    if (signal?.aborted) return
    const gameStore = useGameStore()
    const uiStore = useUIStore()

    gameStore.currentStatus = 'presenting'

    // A new effect owns the layer; stale timers/listeners may no longer mutate it.
    detachCurrentAbort()
    if (activeTimer !== null) {
      window.clearTimeout(activeTimer)
      activeTimer = null
    }

    // BSOD 的剧本自带彩蛋文本（trace 行/独白）；切到非 BSOD 特效时清掉
    if (event.effect.split('+').includes('BSOD')) {
      uiStore.bsodText = event.text ?? ''
      uiStore.bsodEcho = event.echo ?? ''
    } else if (uiStore.bsodText || uiStore.bsodEcho) {
      uiStore.bsodText = ''
      uiStore.bsodEcho = ''
    }

    const previous = useSettingsStore().display.backgroundEffect
    const mySeq = ++effectFlashSeq
    uiStore.setBackgroundEffect(event.effect)

    if (signal) {
      const onAbort = () => {
        if (mySeq !== effectFlashSeq) return
        effectFlashSeq += 1
        if (activeTimer !== null) {
          window.clearTimeout(activeTimer)
          activeTimer = null
        }
        // Error/manual exit must remove the active horror layer immediately;
        // restoring `previous` could resurrect an older timed horror effect.
        uiStore.resetHorrorEffects()
      }
      signal.addEventListener('abort', onAbort, { once: true })
      detachAbort = () => signal.removeEventListener('abort', onAbort)
    }

    const duration = event.duration
    if (duration > 0) {
      activeTimer = window.setTimeout(() => {
        activeTimer = null
        detachCurrentAbort()
        if (mySeq !== effectFlashSeq) return
        const current = useSettingsStore().display.backgroundEffect
        if (current === event.effect) {
          uiStore.setBackgroundEffect(previous || 'None')
        }
      }, duration * 1000)
    }
  }
}
