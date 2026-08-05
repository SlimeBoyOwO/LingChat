<template>
  <StartList responsive>
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
      <StartItem @click="exitGame">{{ $t('views.menu.exitGame') }}</StartItem>
    </StartLine>
  </StartList>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { useDialogStore } from '@/stores/modules/ui/dialog'  // 保留 Current
import { StartItem, StartLine, StartList } from '../base'    // 保留 Incoming

const emit = defineEmits<{
  (e: 'start-game'): void
  (e: 'open-settings', tab?: string): void
  (e: 'open-credits'): void
  (e: 'open-workshop'): void
}>()

// 保留 Current 的退出逻辑
async function exitGame() {
  const dialogStore = useDialogStore()
  const ok = await dialogStore.confirm('确定要退出游戏吗？', '退出确认')
  if (ok) {
    invoke('exit_app')
  }
}
</script>
