// 文件投喂相关逻辑

import { ref, onMounted, onUnmounted } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useUIStore } from '@/stores/modules/ui/ui'
import { invoke } from '@tauri-apps/api/core'

const IMAGE_EXTS = new Set(['.png', '.jpg', '.jpeg', '.gif', '.webp', '.bmp'])

function isImageFile(path: string): boolean {
  return IMAGE_EXTS.has(path.slice(path.lastIndexOf('.')).toLowerCase())
}

export function useFileDrop() {
  const isDragging = ref(false)
  const ui = useUIStore()
  let unlisten: (() => void) | null = null

  onMounted(async () => {
    unlisten = await getCurrentWindow().onDragDropEvent(async (event) => {
      if (event.payload.type === 'enter' || event.payload.type === 'over') {
        isDragging.value = true
      } else if (event.payload.type === 'leave') {
        isDragging.value = false
      } else if (event.payload.type === 'drop') {
        isDragging.value = false
        const paths = event.payload.paths

        if (paths.length > 1) {
          ui.showNotification({
            type: 'error',
            title: '不支持多个文件',
            message: '呜啊！不要一次塞这么多啦~',
            duration: 2500,
            skipTipsCheck: true,
          })
          return
        }

        const path = paths[0]
        if (isImageFile(path)) {
          try {
            await invoke('pet_feed_image', { path })
            ui.showNotification({ type: 'success', title: '投喂成功！', duration: 2000, skipTipsCheck: true })
          } catch (e) {
            console.error('投喂失败:', e)
          }
        }

        
      }
    })
  })

  onUnmounted(() => {
    unlisten?.()
  })

  return { isDragging }
}