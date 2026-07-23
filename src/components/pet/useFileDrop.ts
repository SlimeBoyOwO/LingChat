// 文件投喂相关逻辑

import { ref, onMounted, onUnmounted } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useUIStore } from '@/stores/modules/ui/ui'
import { invoke } from '@tauri-apps/api/core'
import { readFile } from '@tauri-apps/plugin-fs'

const IMAGE_EXTS = new Set(['.png', '.jpg', '.jpeg', '.gif', '.webp', '.bmp'])

function isImageFile(path: string): boolean {
  return IMAGE_EXTS.has(path.slice(path.lastIndexOf('.')).toLowerCase())
}

async function readAsText(path: string): Promise<string | null> {
  try {
    const bytes = await readFile(path)
    const head = bytes.slice(0, 512)
    if (head.includes(0)) return null
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes)
  } catch {
    return null
  }
}

export function useFileDrop() {
  const isDragging = ref(false)
  const hasFile = ref(false)
  const ui = useUIStore()
  let unlisten: (() => void) | null = null

  onMounted(async () => {
    unlisten = await getCurrentWindow().onDragDropEvent(async (event) => {
      if (event.payload.type === 'enter' || event.payload.type === 'over') {
        isDragging.value = true
        return
      }

      if (event.payload.type === 'leave') {
        isDragging.value = false
        return
      }

      // drop
      isDragging.value = false
      const paths = event.payload.paths


      if (paths.length > 1) {
        ui.showNotification({ type: 'error', title: '不支持多个文件', message: '呜啊！不要一次塞这么多啦~', duration: 2500, skipTipsCheck: true })
        return
      }

      const path = paths[0]

      // 图片
      if (isImageFile(path)) {
        hasFile.value = true
        try {
          await invoke('feed_image', { path })
          ui.showNotification({ type: 'success', title: '投喂成功！', duration: 2000, skipTipsCheck: true })
        } catch (e) {
          hasFile.value = false
          console.error('投喂失败:', e)
        }
        return
      }

      // 文本
      const text = await readAsText(path)
      if (text === null) {
        ui.showNotification({ type: 'error', title: '不支持的文件类型', message: '呜啊！这个文件我看不懂啦~', duration: 2500, skipTipsCheck: true })
        return
      }

      hasFile.value = true
      try {
        await invoke('feed_text', { text })
        ui.showNotification({ type: 'success', title: '文本投喂成功！', duration: 2000, skipTipsCheck: true })
      } catch (e) {
        hasFile.value = false
        console.error('投喂失败:', e)
      }
    })
  })

  onUnmounted(() => {
    unlisten?.()
  })

  return { isDragging, hasFile }
}