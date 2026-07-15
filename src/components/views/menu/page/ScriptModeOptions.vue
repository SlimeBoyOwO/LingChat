<template>
  <nav class="flex flex-col items-stretch">
    <StartItem
      v-for="(script, index) in currentPageScripts"
      :key="script.script_name"
      @click="selectScript(script)"
    >
      {{ script.script_name }}
    </StartItem>

    <!-- 占位 -->
    <StartItem v-for="n in pageSize - currentPageScripts.length" :key="'placeholder-' + n" disabled>
      {{ '\u00A0' }}
    </StartItem>

    <!-- 分页控制 -->
    <div>
      <StartItem @click="currentPage--" :disabled="currentPage === 1"><</StartItem>
      <StartItem disabled style="font-size: 28px">{{ currentPage }} / {{ totalPages }}</StartItem>
      <StartItem @click="currentPage++" :disabled="currentPage === totalPages">></StartItem>
      <!-- 返回按钮 -->
      <StartItem @click="backToGameModeMenu">返回</StartItem>
    </div>
  </nav>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { StartItem } from '../base'
import { useRouter } from 'vue-router'
import { type ScriptSummary, startScript } from '@/api/services/script-info'
import { useGameStore } from '@/stores/modules/game'

const emit = defineEmits<{
  (e: 'back'): void
}>()

const props = defineProps({
  scripts: {
    type: Array as () => ScriptSummary[],
    default: [],
  },
})

const router = useRouter()
const gameStore = useGameStore()

const currentPage = ref(1)
const pageSize = 3

const selectScript = async (script: ScriptSummary) => {
  await router.push('/chat')

  gameStore.enterStoryMode(script.script_name)

  await startScript(script.script_name)
}

const backToGameModeMenu = () => {
  emit('back')
}

const totalPages = computed(() => {
  return Math.ceil(props.scripts.length / pageSize)
})

const currentPageScripts = computed(() => {
  const start = (currentPage.value - 1) * pageSize
  const end = start + pageSize
  return props.scripts.slice(start, end)
})
</script>
