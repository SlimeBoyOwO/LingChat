import { onMounted, onUnmounted } from 'vue'
import { useAsrInput } from './useAsrInput'
import { useAsrStore } from '@/stores/modules/settings/asr'

/**
 * 解析 KeyboardEvent 为 "Ctrl+Shift+Space" 形式的快捷键字符串。
 */
export function eventToCombo(e: KeyboardEvent): string {
  const parts: string[] = []
  if (e.ctrlKey) parts.push('Ctrl')
  if (e.shiftKey) parts.push('Shift')
  if (e.altKey) parts.push('Alt')
  // 优先 e.code（位置无关，'KeyA' / 'Space'），fallback e.key（'a' / ' '）
  let key = e.code
  if (!key || key === 'Unidentified') {
    key = e.key
  }
  // 简化 'KeyA' → 'A'
  key = key.replace(/^Key/, '').toLowerCase()
  // Space 保留
  if (e.code === 'Space') key = 'space'
  parts.push(key)
  return parts.join('+')
}

/**
 * 判断 KeyboardEvent 是否匹配 combo 字符串。
 */
export function matchCombination(e: KeyboardEvent, combo: string): boolean {
  const parts = combo.split('+').map((s) => s.trim().toLowerCase())
  const needCtrl = parts.includes('ctrl') || parts.includes('control')
  const needShift = parts.includes('shift')
  const needAlt = parts.includes('alt')
  const last = parts[parts.length - 1]
  if (!last) return false
  if (e.ctrlKey !== needCtrl) return false
  if (e.shiftKey !== needShift) return false
  if (e.altKey !== needAlt) return false
  const evKey = (e.code || e.key).toLowerCase().replace(/^key/, '')
  const evSpace = e.code === 'Space' ? 'space' : evKey
  return evSpace === last
}

/**
 * 录制快捷键：捕获下一次 keydown（除 Escape），转 combo 字符串返回。
 * Escape 取消录制（返回空字符串）。
 * 由 SettingsAsr.vue 的"录制快捷键"按钮触发。
 */
export async function recordKeyUntilEscape(): Promise<string> {
  return new Promise((resolve) => {
    const handler = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()
      if (e.key === 'Escape') {
        cleanup()
        resolve('')
      } else {
        cleanup()
        resolve(eventToCombo(e))
      }
    }
    const cleanup = () => {
      window.removeEventListener('keydown', handler, true)
    }
    window.addEventListener('keydown', handler, true)
  })
}

/**
 * 全局快捷键 composable：在 chatActive 时监听 asrStore.settings.hotkey_combination。
 * 装在 App 根组件（如 App.vue 或 main.ts 挂载）一次即可。
 */
export function useGlobalHotkey() {
  const asr = useAsrInput()
  const asrStore = useAsrStore()

  const handler = (e: KeyboardEvent) => {
    if (!asr.chatActive.value) return
    if (!asrStore.settings.hotkey_enabled) return
    if (!matchCombination(e, asrStore.settings.hotkey_combination)) return
    e.preventDefault()
    if (e.type === 'keydown' && !e.repeat) {
      void asr.start('hotkey')
    } else if (e.type === 'keyup') {
      asr.stop()
    }
  }

  onMounted(() => {
    window.addEventListener('keydown', handler)
    window.addEventListener('keyup', handler)
  })
  onUnmounted(() => {
    window.removeEventListener('keydown', handler)
    window.removeEventListener('keyup', handler)
  })
}