<template>
  <nav class="flex flex-col items-stretch">
    <StartItem @click="startFreeDialogue">自由对话模式</StartItem>
    <StartItem @click="startStoryMode" disabled>剧情模式（即将登场）</StartItem>
    <StartItem disabled>小游戏（开发中）</StartItem>
    <StartItem @click="() => emit('back')">返回</StartItem>
  </nav>
</template>

<script setup lang="ts">
import { StartItem } from '../base'
import { useRouter } from 'vue-router'
import { useGameStore } from '@/stores/modules/game'
import { applyWebInitData } from '@/stores/modules/game/actions'
import type { WebInitData } from '@/api/services/game-info'
import { invoke } from '@tauri-apps/api/core'

const emit = defineEmits<{
  (e: 'back'): void
  (e: 'open-scripts'): void
}>()

const router = useRouter()
const gameStore = useGameStore()

// galgame New Game：清空当前进行，开新世界。必须调 start_new_game，
// 否则 /chat 页会走 init_game 自动恢复上次会话，"开始游戏"就名不副实了。
const startFreeDialogue = async () => {
  gameStore.exitStoryMode()
  try {
    const data = await invoke<WebInitData>('start_new_game')
    applyWebInitData(gameStore.$state, data)
  } catch (e) {
    console.error('开始新游戏失败:', e)
  }
  router.push('/chat')
}

// 前端进入剧情模式（开发中）

const startStoryMode = async () => {
  emit('open-scripts')
}
</script>
