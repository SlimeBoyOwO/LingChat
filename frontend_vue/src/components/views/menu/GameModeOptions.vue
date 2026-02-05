<template>
  <nav class="flex flex-col items-stretch w-[350px]">
    <button
      v-for="item in menuItems"
      :key="item.label"
      class="menu-item"
      :class="{ 'menu-item--disabled': item.disabled }"
      :disabled="item.disabled"
      @click="item.action"
    >
      {{ item.label }}
    </button>
  </nav>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { getScriptList, type ScriptSummary } from '@/api/services/script-info'
import { scriptHandler } from '@/api/websocket/handlers/script-handler'
import { useUIStore } from '@/stores/modules/ui/ui'
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

interface MenuItem {
  label: string
  action: () => void
  disabled?: boolean
}

const startFreeDialogue = () => {
  gameStore.exitStoryMode()
  router.push('/chat')
}

//前端进入剧情模式（开发中）

const startStoryMode = async () => {
  emit('open-scripts')
}

const menuItems = computed<MenuItem[]>(() => [
  { label: '自由对话模式', action: startFreeDialogue },
  {
    label: '剧情模式',
    action: startStoryMode,
    disabled: props.loadingScripts || props.scripts.length === 0,
  },
  { label: '小游戏', action: () => {}, disabled: true },
  { label: '返回', action: () => emit('back') },
])
</script>

<style scoped>
@import './menu-item.css';
</style>
