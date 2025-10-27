<template>
  <router-view />
  <CursorEffects />
</template>

<script setup>
import { onMounted, onUnmounted } from 'vue';
import CursorEffects from "./components/effects/CursorEffects.vue";
// 在使用 <router-view> 的情况下，通常不需要在这里再导入具体的页面组件了

const handleKeyDown = (event) => {
  if (event.key === 'F11') {
    event.preventDefault();
    if (window.pywebview && window.pywebview.api && typeof window.pywebview.api.toggle_fullscreen === 'function') {
      // 调用从 Python 暴露的函数
      window.pywebview.api.toggle_fullscreen();
    } else {
      console.error('全屏API不可用。');
    }
  }
};

onMounted(() => {
  // 等待 pywebview API 准备就绪
  window.addEventListener('pywebviewready', () => {
    window.addEventListener('keydown', handleKeyDown);
  });
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
});
</script>

<style>
:root {
  /*全局变量*/
  --accent-color: #79d9ff;
  --menu-max-width: 1100px;
  --menu-max-width-half: 550px;
  /* 一个生动的天蓝色，可以根据你的品牌调整 */

  .glass-effect {
    background: rgba(255, 255, 255, 0.1);
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    border: 1px solid rgba(255, 255, 255, 0.125);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1),
      inset 0 1px 1px rgba(255, 255, 255, 0.1);
    transition: border-color 0.2s, box-shadow 0.2s;
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
