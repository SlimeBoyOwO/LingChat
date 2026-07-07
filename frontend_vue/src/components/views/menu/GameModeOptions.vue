<template>
  <nav class="flex flex-col items-stretch w-[350px]">
    <StartItem @click="startFreeDialogue" :disabled="false">自由对话模式</StartItem>
    <StartItem @click="startStoryMode" :disabled="true">剧情模式（即将登场）</StartItem>
    <StartItem @click="() => {}" :disabled="true">小游戏（开发中）</StartItem>
    <StartItem @click="() => emit('back')" :disabled="false">返回</StartItem>
  </nav>
</template>

<script setup lang="ts">
import '../../base'
import { useRouter } from 'vue-router'
import { getScriptList, type ScriptSummary } from '@/api/services/script-info'
import { useGameStore } from '@/stores/modules/game'

const emit = defineEmits<{
  (e: 'back'): void
  (e: 'open-scripts'): void
}>()

const props = defineProps({
  scripts: {
    type: Array as () => ScriptSummary[],
    default: [],
  },
  loadingScripts: {
    type: Boolean,
    default: false,
  },
})

const router = useRouter()
const gameStore = useGameStore()

const startFreeDialogue = () => {
  gameStore.exitStoryMode()
  router.push('/chat')
}

//前端进入剧情模式（开发中）

const startStoryMode = async () => {
  emit('open-scripts')
}
</script>

<style scoped></style>
