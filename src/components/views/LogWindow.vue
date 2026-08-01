<template>
  <div class="log-window-root">
    <LogConsole standalone />
  </div>
</template>

<script setup lang="ts">
import LogConsole from '../settings/pages/LogConsole.vue'
</script>

<style scoped>
.log-window-root {
  display: flex;
  flex-direction: column;
  height: 100vh;
  padding: 12px;
  box-sizing: border-box;
  background: #1a1b1e;
}
</style>
