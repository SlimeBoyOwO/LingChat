<template>
  <nav class="flex flex-col items-stretch">
    <StartItem @click="() => emit('start-game')">{{ $t('views.menu.startGame') }}</StartItem>
    <StartItem @click="() => emit('open-settings', 'save')">{{ $t('views.menu.continueGame') }}</StartItem>
    <StartItem @click="() => emit('open-script-editor')">{{ $t('views.menu.scriptEditor') }}</StartItem>
    <StartItem @click="() => emit('open-settings')">{{ $t('views.menu.gameConfig') }}</StartItem>
    <StartItem @click="() => emit('open-credits')">{{ $t('views.menu.credits') }}</StartItem>
    <StartItem @click="exitGame">{{ $t('views.menu.exitGame') }}</StartItem>
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
  (e: 'open-script-editor'): void
}>()

// 退出游戏
// 先弹确认框，确认后用 Rust 端 exit_app 命令（app.exit()），桌面和 Android 都有效，
// 避免手机上误触退出丢失进度。
async function exitGame() {
  const dialogStore = useDialogStore()
  const ok = await dialogStore.confirm('确定要退出游戏吗？', '退出确认')
  if (ok) {
    invoke('exit_app')
  }
}
</script>
