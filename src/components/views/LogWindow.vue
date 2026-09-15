<template>
  <!-- h-full 而非 h-dvh：useZoom 的 transform: scale 会把 #app 布局高度设为
       100dvh/z（localStorage 跨窗口共享缩放级别），h-dvh 在 z≠1 时填不满 #app，
       底部会露出默认白底（本窗口曾出现的「底部白边」） -->
  <div class="box-border flex h-full min-h-0 flex-col bg-[#1a1b1e] p-3">
    <LogConsole standalone />
  </div>
</template>

<script setup lang="ts">
  import LogConsole from "../settings/pages/LogConsole.vue";
</script>
