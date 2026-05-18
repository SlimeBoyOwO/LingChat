import { listen } from '@tauri-apps/api/event'
import { eventQueue } from '../core/events/event-queue'
import type { ScriptEventType } from '../types'

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
    eventQueue.addEvent(asEvent(event.payload, { type: 'script_end', duration: 0 }))
  })

  listen('script:free-dialogue', (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: 'free_dialogue', duration: 0 }))
  })

  console.log('[Tauri] Event listeners initialized (ai:reply, ai:thinking, ai:error, status:reset + 13 script events)')
}
