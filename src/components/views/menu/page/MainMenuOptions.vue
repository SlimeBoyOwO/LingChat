<template>
  <nav class="flex flex-col items-stretch">
    <StartItem @click="() => emit('start-game')">开始游戏</StartItem>
    <StartItem @click="() => emit('open-settings', 'save')">继续游戏</StartItem>
    <StartItem @click="() => emit('open-script-editor')">剧本编辑器</StartItem>
    <StartItem @click="() => emit('open-settings')">游戏配置</StartItem>
    <StartItem @click="() => emit('open-credits')">致谢名单</StartItem>
    <StartItem @click="exitGame">退出游戏</StartItem>
  </nav>
</template>

<script setup lang="ts">
import { StartItem } from '../base'
import { invoke } from '@tauri-apps/api/core'

const emit = defineEmits<{
  (e: 'start-game'): void
  (e: 'open-settings', tab?: string): void
  (e: 'open-credits'): void
  (e: 'open-script-editor'): void
}>()

// 退出游戏
function exitGame() {
  invoke('exit_app')
}
</script>
