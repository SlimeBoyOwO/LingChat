<template>
  <nav class="flex flex-col items-stretch w-87.5">
    <button v-for="item in menuItems" :key="item.label" class="menu-item" @click="item.action">
      {{ item.label }}
    </button>
  </nav>
</template>

<script setup lang="ts">
const emit = defineEmits<{
  (e: 'start-game'): void
  (e: 'open-settings', tab?: string): void
  (e: 'open-credits'): void
}>()

interface MenuItem {
  label: string
  action: () => void
}

const menuItems: MenuItem[] = [
  { label: '开始游戏', action: () => emit('start-game') },
  { label: '继续游戏', action: () => emit('open-settings', 'save') },
  { label: '设置', action: () => emit('open-settings') },
  { label: '致谢', action: () => emit('open-credits') },
  { label: '退出游戏', action: () => window.close() },
]
</script>

<style scoped>
@import './menu-item.css';
</style>
