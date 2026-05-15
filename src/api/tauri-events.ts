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

  console.log('[Tauri] Event listeners initialized (ai:reply, ai:thinking, ai:error, status:reset)')
}
