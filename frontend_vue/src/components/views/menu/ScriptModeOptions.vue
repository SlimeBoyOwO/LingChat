<template>
  <nav class="flex flex-col items-stretch w-87.5">
    <button
      v-for="(script, index) in currentPageScripts"
      :key="script.script_name"
      class="menu-item"
      @click="selectScript(script)"
    >
      {{ script.script_name }}
    </button>

    <!-- 占位 -->
    <button
      v-for="n in pageSize - currentPageScripts.length"
      :key="'placeholder-' + n"
      class="menu-item menu-item--disabled"
      disabled
    >
      {{ '\u00A0' }}
    </button>

    <!-- 分页控制 -->
    <div class="pagination-controls">
      <button class="menu-item" :disabled="currentPage === 1" @click="currentPage--"><</button>
      <span class="menu-item" style="font-size: 28px">{{ currentPage }} / {{ totalPages }}</span>
      <button class="menu-item" :disabled="currentPage === totalPages" @click="currentPage++">
        >
      </button>
      <!-- 返回按钮 -->
      <button key="back" class="menu-item" @click="backToGameModeMenu">返回</button>
    </div>
  </nav>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { type ScriptSummary } from '@/api/services/script-info'
import { scriptHandler } from '@/api/websocket/handlers/script-handler'
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

interface MenuItem {
  label: string
  action: () => void
  disabled?: boolean
}

const selectScript = async (script: ScriptSummary) => {
  await router.push('/chat')

  const command = `/开始剧本 ${script.script_name}`
  gameStore.enterStoryMode(script.script_name)

  scriptHandler.sendMessage(command)
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

<style scoped>
@import './menu-item.css';
</style>
