<template>
  <nav class="flex flex-col items-stretch">
    <StartItem @click="() => emit('start-game')">{{ $t('views.menu.startGame') }}</StartItem>
    <StartItem @click="() => emit('open-settings', 'save')">{{ $t('views.menu.continueGame') }}</StartItem>
    <StartItem @click="() => emit('open-settings')">{{ $t('views.menu.gameConfig') }}</StartItem>
    <StartItem @click="() => emit('open-credits')">{{ $t('views.menu.credits') }}</StartItem>
    <StartItem @click="exitGame">{{ $t('views.menu.exitGame') }}</StartItem>
  </nav>
</template>

<script setup lang="ts">
import { StartItem } from '../base'
import { invoke } from '@tauri-apps/api/core'

const emit = defineEmits<{
  (e: 'start-game'): void
  (e: 'open-settings', tab?: string): void
  (e: 'open-credits'): void
}>()

// 退出游戏
function exitGame() {
  invoke('exit_app')
}
</script>
