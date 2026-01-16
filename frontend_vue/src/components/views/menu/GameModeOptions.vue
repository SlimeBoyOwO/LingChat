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
import { useRouter } from 'vue-router'

const emit = defineEmits<{
  (e: 'back'): void
}>()

const router = useRouter()

interface MenuItem {
  label: string
  action: () => void
  disabled?: boolean
}

const startFreeDialogue = () => {
  router.push('/chat')
}

const menuItems: MenuItem[] = [
  { label: '自由对话模式', action: startFreeDialogue },
  { label: '剧情模式', action: () => {}, disabled: true },
  { label: '小游戏', action: () => {}, disabled: true },
  { label: '返回', action: () => emit('back') },
]
</script>

<style scoped>
@import './menu-item.css';
</style>
