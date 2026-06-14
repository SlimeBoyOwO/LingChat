import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { eventQueue } from '../core/events/event-queue'
import type { ScriptEventType } from '../types'
import { useAdventureStore } from '../stores/modules/adventure'
import { useUIStore } from '../stores/modules/ui/ui'
import { useGameStore } from '../stores/modules/game'

function asEvent(payload: unknown, overrides: Partial<ScriptEventType>): ScriptEventType {
  return { ...(payload as Record<string, unknown>), ...overrides } as unknown as ScriptEventType
}

export function initializeTauriEventListeners() {
  listen('ai:reply', (event) => {
    console.log('[Tauri] ai:reply', event.payload)
    eventQueue.addEvent(asEvent(event.payload, { type: 'reply', duration: -1 }))
  })

  listen('ai:thinking', (event) => {
    console.log('[Tauri] ai:thinking', event.payload)
    eventQueue.addEvent(asEvent(event.payload, { type: 'thinking', duration: 0 }))
  })

  listen('ai:error', (event) => {
    const p = event.payload as Record<string, unknown>
    console.log('[Tauri] ai:error', p)
    eventQueue.addEvent({
      type: 'error',
      duration: 0,
      error_code: (p.error_code as string) ?? 'default_error',
      message: (p.detail as string) ?? '',
    } as ScriptEventType)
  })

  listen('status:reset', (event) => {
    console.log('[Tauri] status:reset', event.payload)
    eventQueue.addEvent(asEvent(event.payload, { type: 'status_reset', duration: 0 }))
  })

  // === Adventure events ===

  listen('adventure:unlocked', (event) => {
    const payload = event.payload as any
    console.log('[Tauri] adventure:unlocked', payload)
    const adventureStore = useAdventureStore()
    if (payload?.adventure_folder) {
      adventureStore.unlockNotifications.push(payload)
    }
  })

  listen('adventure:completed', (event) => {
    const payload = event.payload as any
    console.log('[Tauri] adventure:completed', payload)
    const adventureStore = useAdventureStore()
    if (payload?.adventure_folder) {
      adventureStore.markAdventureCompleted(payload.adventure_folder)
    }
  })

  // === Auto-save events ===

  listen('save:auto-saved', async (event) => {
    const payload = event.payload as { save_id: number; title: string; timestamp: string }
    console.log('[Tauri] save:auto-saved', payload)

    // Capture screenshot for auto-save slot
    const gameStore = useGameStore()
    const screenshotPath = await gameStore.captureScreenshot()
    if (screenshotPath) {
      try {
        await invoke('save_screenshot', {
          saveId: payload.save_id,
          screenshotPath,
        })
      } catch (e) {
        console.error('[Tauri] Failed to save auto-save screenshot', e)
      }
    }

    useUIStore().showNotification({
      type: 'info',
      title: '自动存档',
      message: `已于 ${payload.timestamp} 自动保存`,
      duration: 2500,
      skipTipsCheck: true,
    })
  })

  // === Script events ===

  listen('script:narration', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'narration', duration: -1 }))
  })

  listen('script:player', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'player', duration: -1 }))
  })

  listen('script:chapter-change', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'chapter_change', duration: 0 }))
  })

  listen('script:background', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'background', duration: 0 }))
  })

  listen('script:background-effect', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'background_effect', duration: 0 }))
  })

  listen('script:music', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'music', duration: 0 }))
  })

  listen('script:sound', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'sound', duration: 0 }))
  })

  listen('script:present-pic', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'present_pic', duration: -1 }))
  })

  listen('script:modify-character', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'modify_character', duration: 0 }))
  })

  listen('script:input', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'input', duration: 0 }))
  })

  listen('script:choice', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'choice', duration: 0 }))
  })

  listen('script:end', (event) => {
    console.log('[Tauri] script:end', event.payload)
    eventQueue.addEvent(asEvent(event.payload, { type: 'script_end', duration: 0, isFinal: true }))
  })

  listen('script:free-dialogue', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'free_dialogue', duration: 0 }))
  })

  // === God Agent multi-dialogue event ===

  listen('character:switch', (event) => {
    const payload = event.payload as { type: string; roleId: number; characterName: string }
    console.log('[Tauri] character:switch', payload)
    const gameStore = useGameStore()
    gameStore.currentInteractRoleId = payload.roleId
    // Ensure the role is loaded in gameRoles
    gameStore.getOrCreateGameRole(payload.roleId)
  })

  console.log('[Tauri] Event listeners initialized (ai + adventure + auto-save + 13 script events + character:switch)')
}
