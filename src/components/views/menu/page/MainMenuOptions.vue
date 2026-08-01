<template>
  <StartList>
    <StartLine>
      <StartItem @click="() => emit('start-game')">开始游戏</StartItem>
    </StartLine>
    <StartLine>
      <StartItem @click="() => emit('open-settings', 'save')">继续游戏</StartItem>
    </StartLine>
    <StartLine :mobile="false">
      <StartItem @click="() => emit('open-workshop')">创意工坊</StartItem>
    </StartLine>
    <StartLine>
      <StartItem @click="() => emit('open-settings')">游戏配置</StartItem>
    </StartLine>
    <StartLine>
      <StartItem @click="() => emit('open-credits')">致谢名单</StartItem>
    </StartLine>
    <StartLine>
      <StartItem @click="invoke('exit_app')">退出游戏</StartItem>
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
