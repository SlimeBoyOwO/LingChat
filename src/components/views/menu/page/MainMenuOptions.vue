<template>
  <StartList>
    <StartLine>
      <StartItem @click="() => emit('start-game')">{{ $t('views.menu.startGame') }}</StartItem>
    </StartLine>
    <StartLine>
      <StartItem @click="() => emit('open-settings', 'save')">{{ $t('views.menu.continueGame') }}</StartItem>
    </StartLine>
    <StartLine :mobile="false">
      <StartItem @click="() => emit('open-workshop')">{{ $t('views.menu.scriptEditor') }}</StartItem>
    </StartLine>
    <StartLine>
      <StartItem @click="() => emit('open-settings')">{{ $t('views.menu.gameConfig') }}</StartItem>
    </StartLine>
    <StartLine>
      <StartItem @click="() => emit('open-credits')">{{ $t('views.menu.credits') }}</StartItem>
    </StartLine>
    <StartLine>
      <StartItem @click="invoke('exit_app')">{{ $t('views.menu.exitGame') }}</StartItem>
    </StartLine>
  </StartList>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { StartItem, StartLine, StartList } from '../base'

const emit = defineEmits<{
  (e: 'start-game'): void
  (e: 'open-settings', tab?: string): void
  (e: 'open-credits'): void
  (e: 'open-workshop'): void
}>()
</script>
