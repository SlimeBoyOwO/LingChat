<!-- src/App.vue -->
<template>
  <div id="app">
    <!-- 可能的导航菜单 -->
    <router-view></router-view>
  </div>
</template>

<script setup>
// 你的逻辑
</script>

<style lang="css">
/* 关键：让整个应用背景透明，并充满窗口 */
html,
body,
#app {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  background: transparent !important; /* 确保背景透明 */
  overflow: hidden; /* 隐藏可能出现的滚动条 */
}

:root {
  --accent-color: #79d9ff;
}
</style>
