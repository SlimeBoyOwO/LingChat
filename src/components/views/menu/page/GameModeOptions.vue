<template>
  <StartList>
    <StartLine>
      <StartItem @click="startFreeDialogue">自由对话模式</StartItem>
    </StartLine>

    <StartLine>
      <StartItem @click="emit('open-scripts')">剧情演绎模式</StartItem>
    </StartLine>

    <StartLine>
      <StartItem disabled="true">游戏互动模式（开发中）</StartItem>
    </StartLine>

    <StartLine>
      <StartItem @click="emit('back')">返回</StartItem>
    </StartLine>
  </StartList>
</template>

<script setup lang="ts">
import { StartItem, StartLine, StartList } from '../base'
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
</script>
