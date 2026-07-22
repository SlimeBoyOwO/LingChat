import { ref, onMounted, onUnmounted } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useUIStore } from '@/stores/modules/ui/ui'

export function useFileDrop() {
  const isDragging = ref(false)
  const ui = useUIStore()

  let unlisten: (() => void) | null = null

  onMounted(async () => {
    unlisten = await getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload.type === 'enter' || event.payload.type === 'over') {
        isDragging.value = true
      } else if (event.payload.type === 'leave') {
        isDragging.value = false
      } else if (event.payload.type === 'drop') {
        isDragging.value = false
        const paths = event.payload.paths
        console.log('拖入文件:', paths)
        ui.showNotification({
          type: 'success',
          title: '投喂成功',
          message: `收到 ${paths.length} 个文件:\n${paths.join('\n')}`,
          duration: 5000,
          skipTipsCheck: true,
        })
      }
    })
  })

  onUnmounted(() => {
    unlisten?.()
  })

  return { isDragging }
}