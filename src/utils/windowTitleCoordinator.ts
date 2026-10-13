import { getCurrentWindow } from '@tauri-apps/api/window'

const DEFAULT_TITLE = 'LingChat'
const HORROR_TITLE = 'L⃞i⃟n⃗g⃘C⃙h⃚a⃝t⃞'

let explicitTitle = ''
let horrorActive = false
let appliedTitle = DEFAULT_TITLE
let updateChain: Promise<void> = Promise.resolve()

function desiredTitle(): string {
  if (horrorActive) return HORROR_TITLE
  return explicitTitle || DEFAULT_TITLE
}

function scheduleTitleUpdate(): void {
  const title = desiredTitle()
  if (title === appliedTitle) return
  appliedTitle = title
  updateChain = updateChain
    .catch(() => {})
    .then(() => getCurrentWindow().setTitle(title))
    .catch((error) => {
      console.warn('[WindowTitleCoordinator] 设置窗口标题失败:', error)
    })
}

/** 剧本显式 window_title 意图；空串表示回到默认标题。 */
export function setScriptWindowTitle(title: string): void {
  explicitTitle = title.trim()
  scheduleTitleUpdate()
}

/** 恐怖画面临时抢占标题；释放后恢复显式剧本标题而不是盲目恢复 LingChat。 */
export function setHorrorWindowTitleActive(active: boolean): void {
  horrorActive = active
  scheduleTitleUpdate()
}

/** 剧本结束/停止/卸载时清除所有标题意图。 */
export function resetScriptWindowTitle(): void {
  explicitTitle = ''
  horrorActive = false
  scheduleTitleUpdate()
}
