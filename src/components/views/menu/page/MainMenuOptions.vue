<template>
  <nav class="flex flex-col items-stretch">
    <StartItem @click="() => emit('start-game')">开始游戏</StartItem>
    <StartItem @click="() => emit('open-settings')">游戏配置</StartItem>
    <StartItem @click="() => emit('open-credits')">致谢名单</StartItem>
    <StartItem @click="exitGame">退出游戏</StartItem>
  </nav>
</template>

<script setup lang="ts">
import { StartItem } from '../base'
import { invoke } from '@tauri-apps/api/core'
import { useDialogStore } from '@/stores/modules/ui/dialog'

const emit = defineEmits<{
  (e: 'start-game'): void
  (e: 'open-settings', tab?: string): void
  (e: 'open-credits'): void
}>()

// 退出游戏
// 先弹确认框，确认后用 Rust 端 exit_app 命令（app.exit()），桌面和 Android 都有效
async function exitGame() {
  const dialogStore = useDialogStore()
  const ok = await dialogStore.confirm('确定要退出游戏吗？', '退出确认')
  if (ok) {
    invoke('exit_app')
  }
}
</script>
