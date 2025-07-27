<template>
  <div class="main_container">
    <SettingsPanel />
  </div>
</template>

<script setup lang="ts">
import SettingsPanel from "../settings/SettingsPanel.vue";
</script>

<style>
.settings-panel {
  position: fixed;
  top: 0;
  right: 0;
  width: 100%;
  height: 100%;
  opacity: 1;
  padding: 0;
  box-sizing: border-box;
  z-index: 1000;
  color: #333;
  /* background-color: rgba(0, 0, 0, 0.25); */
  background-color: transparent;
}

.main_container::before {
  content: "";
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background-image: url("@/assets/images/background.png");
  background-size: cover;
  background-position: center;
  z-index: 0;
  filter: blur(0px) brightness(1);
  transition: filter 0.6s cubic-bezier(0.7, 0, 0.2, 1);
  pointer-events: none;

  /*TODO 这个高斯效果只有在设置打开的时候启用*/
  filter: blur(12px) brightness(0.9);
}

.container {
  height: 82%;
}
</style>
