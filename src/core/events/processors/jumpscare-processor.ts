import { convertFileSrc } from '@tauri-apps/api/core'
import type { IEventProcessor } from '../event-processor'
import type { ScriptJumpscareEvent } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'

const PRELOAD_TIMEOUT_MS = 1200

async function preloadJumpscare(path: string): Promise<void> {
  if (typeof Image === 'undefined') return
  const src = path.startsWith('http') || path.startsWith('data:') || path.startsWith('blob:')
    ? path
    : convertFileSrc(path)
  await new Promise<void>((resolve) => {
    const image = new Image()
    let settled = false
    const finish = () => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      resolve()
    }
    const timer = window.setTimeout(finish, PRELOAD_TIMEOUT_MS)
    image.onload = finish
    image.onerror = finish
    image.src = src
  })
}

export default class JumpscareProcessor implements IEventProcessor {
  canHandle(eventType: string): boolean {
    return eventType === 'jumpscare'
  }

  async processEvent(event: ScriptJumpscareEvent, signal?: AbortSignal): Promise<void> {
    const gameStore = useGameStore()
    const uiStore = useUIStore()

    gameStore.currentStatus = 'presenting'

    if (!event.imagePath) return
    // Decode the local image before starting the visible timer. Without this,
    // very short scares can expire while the first frame is still loading.
    await preloadJumpscare(event.imagePath)
    if (signal?.aborted) return
    // Store and queue must agree on the effective duration, including the 150ms
    // safety floor, so the next event cannot clear a still-visible overlay.
    const effectiveDuration = Math.max(0.15, event.duration ?? 0.6)
    event.duration = effectiveDuration
    uiStore.triggerJumpscare(event.imagePath, event.soundPath || '', effectiveDuration)
  }
}
