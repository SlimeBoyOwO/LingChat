<template>
  <nav class="flex flex-col items-stretch">
    <StartItem @click="startFreeDialogue">{{ $t('views.menu.freeDialogue') }}</StartItem>
    <StartItem @click="startStoryMode" disabled>{{ $t('views.menu.storyMode') }}</StartItem>
    <StartItem disabled>{{ $t('views.menu.miniGame') }}</StartItem>
    <StartItem @click="() => emit('back')">{{ $t('views.menu.back') }}</StartItem>
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
