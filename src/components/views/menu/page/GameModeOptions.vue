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

const emit = defineEmits<{
  (e: 'back'): void
  (e: 'open-scripts'): void
}>()

const router = useRouter()
const gameStore = useGameStore()

const startFreeDialogue = () => {
  gameStore.exitStoryMode()
  router.push('/chat')
}

// 前端进入剧情模式（开发中）

const startStoryMode = async () => {
  emit('open-scripts')
}
</script>
