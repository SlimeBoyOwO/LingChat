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

const emit = defineEmits<{
  (e: 'back'): void
}>()

const router = useRouter()
const uiStore = useUIStore()

const scripts = ref<ScriptSummary[]>([])
const loadingScripts = ref(false)

interface MenuItem {
  label: string
  action: () => void
  disabled?: boolean
}

const startFreeDialogue = () => {
  router.push('/chat')
}


//前端进入剧情模式（开发中） 

const startStoryMode = async () => {
  await router.push('/chat')

  // 默认选择第一个剧本；如果有多个，可在这里做更完善的选择UI
  const chosen = scripts.value[0]?.script_name
  const command = chosen ? `/开始剧本 ${chosen}` : '/开始剧本'

  const ok = scriptHandler.sendMessage(command)
  if (!ok) {
    console.warn('发送开始剧本指令失败，可能后端未启动或 WebSocket 未连接')
  }
}

onMounted(async () => {
  loadingScripts.value = true
  try {
    scripts.value = await getScriptList()
  } catch (e) {
    uiStore.showError({
      errorCode: 'script_list_failed',
      message: '获取剧本列表失败：请确认后端已启动',
    })
    scripts.value = []
  } finally {
    loadingScripts.value = false
  }
})

const menuItems = computed<MenuItem[]>(() => [
  { label: '自由对话模式', action: startFreeDialogue },
  {
    label: '剧情模式',
    action: startStoryMode,
    disabled: loadingScripts.value || scripts.value.length === 0,
  },
  { label: '小游戏', action: () => {}, disabled: true },
  { label: '返回', action: () => emit('back') },
])
</script>

<style scoped>
@import './menu-item.css';
</style>
