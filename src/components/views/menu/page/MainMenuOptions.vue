<template>
  <nav class="flex flex-col items-stretch">
    <StartItem @click="() => emit('start-game')">开始游戏</StartItem>
    <StartItem @click="() => emit('open-settings', 'save')">继续游戏</StartItem>
    <StartItem @click="() => emit('open-settings')">游戏配置</StartItem>
    <StartItem @click="() => emit('open-credits')">致谢名单</StartItem>
    <StartItem @click="exitGame">退出游戏</StartItem>
  </nav>
</template>

<script setup lang="ts">
import { StartItem } from '../base'
import { getCurrentWindow } from '@tauri-apps/api/window'

const emit = defineEmits<{
  (e: 'start-game'): void
  (e: 'open-settings', tab?: string): void
  (e: 'open-credits'): void
}>()

// 退出游戏
function exitGame() {
  getCurrentWindow().destroy()
}
</script>
