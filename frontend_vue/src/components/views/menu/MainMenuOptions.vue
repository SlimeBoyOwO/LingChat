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

// 退出游戏：优先调用 WebView API，如果不可用则回退到 window.close()
function exitGame() {
  // @ts-ignore - pywebview js_api
  if (window.pywebview && window.pywebview.api && typeof window.pywebview.api.exit_app === 'function') {
    window.pywebview.api.exit_app()
  } else {
    window.close()
  }
}

const menuItems: MenuItem[] = [
  { label: '开始游戏', action: () => emit('start-game') },
  { label: '继续游戏', action: () => emit('open-settings', 'save') },
  { label: '设置', action: () => emit('open-settings') },
  { label: '致谢', action: () => emit('open-credits') },
  { label: '退出游戏', action: exitGame },
]
</script>

<style scoped>
@import './menu-item.css';
</style>
