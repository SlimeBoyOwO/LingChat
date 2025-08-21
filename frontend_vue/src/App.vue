<template>
  <MainChat />
  <Settings />
</template>

<script setup lang="ts">
import MainChat from "./components/views/MainChat.vue";
import Settings from "./components/settings/SettingsPanel.vue";
</script>

<style>
/* 定义自定义字体 */
@font-face {
  font-family: "Maoken Assorted Sans";
  src: url("./assets/fonts/MaokenAssortedSans.ttf") format("truetype");
  font-weight: normal;
  font-style: normal;
  font-display: swap;
}

:root {
  /*全局变量*/
  --accent-color: #79d9ff;
  --menu-max-width: 1100px;
  --menu-max-width-half: 550px;
  /* 一个生动的天蓝色，可以根据你的品牌调整 */

  --glass-effect: {
    background: rgba(255, 255, 255, 0.1);
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    border: 1px solid rgba(255, 255, 255, 0.125);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1),
      inset 0 1px 1px rgba(255, 255, 255, 0.1);
  }
}

/* 全局样式和字体 */
body,
html {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto,
    "Helvetica Neue", Arial, sans-serif;
  overflow: hidden;
  background: transparent;
  /* 确保body背景透明，不遮挡我们的背景图 */
}

#app {
  width: 100vw;
  height: 100vh;
}
</style>
