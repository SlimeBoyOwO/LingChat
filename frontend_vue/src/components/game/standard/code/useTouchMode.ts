/**
 * 触摸模式 Composable
 * 管理触摸模式的进入/退出/光标/右键菜单
 */

import { useGameStore } from '@/stores/modules/game'

const TOUCH_CURSOR_SVG =
  `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' ` +
  `viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' ` +
  `stroke-linejoin='round' class='lucide lucide-hand-icon lucide-hand'%3E%3Cpath d='M18 11V6a2 2 ` +
  `0 0 0-2-2a2 2 0 0 0-2 2'/%3E%3Cpath d='M14 10V4a2 2 0 0 0-2-2a2 2 0 0 0-2 2v2'/%3E` +
  `%3Cpath d='M10 10.5V6a2 2 0 0 0-2-2a2 2 0 0 0-2 2v8'/%3E%3Cpath d='M18 8a2 2 0 1 1 4 0v6` +
  `a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.86-5.99-2.34l-3.6-3.6a2 2 0 0 1 2.83-2.82L7 15'/%3E%3C/svg%3E") 0 0, auto`

export function useTouchMode() {
  const gameStore = useGameStore()

  const handleRightClick = (e: MouseEvent) => {
    if (gameStore.command === 'touch') {
      e.preventDefault()
      exitTouchMode()
    }
  }

  function toggleTouchMode() {
    if (gameStore.command === 'touch') {
      exitTouchMode()
      return
    }
    document.body.style.cursor = TOUCH_CURSOR_SVG
    gameStore.command = 'touch'
    document.addEventListener('contextmenu', handleRightClick)
  }

  function exitTouchMode() {
    document.body.style.cursor = 'default'
    if (gameStore.command === 'touch') {
      gameStore.command = null
    }
    document.removeEventListener('contextmenu', handleRightClick)
  }

  function cleanup() {
    exitTouchMode()
  }

  return { toggleTouchMode, exitTouchMode, cleanup }
}
