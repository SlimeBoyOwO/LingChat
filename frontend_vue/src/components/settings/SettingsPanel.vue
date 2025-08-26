<template>
  <div
    class="blur-overlay"
    :style="{ opacity: uiStore.showSettings ? 1 : 0 }"
  ></div>
  <div class="settings-panel" v-show="uiStore.showSettings">
    <div class="header">
      <SettingsNav />
    </div>
    <div class="container">
      <SettingsSave v-show="uiStore.currentSettingsTab === 'save'"/>
      <SettingsText v-show="uiStore.currentSettingsTab === 'text'"/>
      <SettingsSound v-show="uiStore.currentSettingsTab === 'sound'"/>
      <SettingsAdvance v-show="uiStore.currentSettingsTab === 'advance'"/>
      <SettingsHistory v-show="uiStore.currentSettingsTab === 'history'"/>
      <SettingsSchedule v-show="uiStore.currentSettingsTab === 'schedule'"/>
      <SettingsCharacter v-show="uiStore.currentSettingsTab === 'character'"/>
      <SettingsBackground v-show="uiStore.currentSettingsTab === 'background'"/>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  SettingsText,
  SettingsSave,
  SettingsSound,
  SettingsHistory,
  SettingsAdvance,
  SettingsSchedule,
  SettingsCharacter,
  SettingsBackground,
} from './pages'
import SettingsNav from "./SettingsNav.vue";
import { useUIStore } from "../../stores/modules/ui/ui";

const uiStore = useUIStore();
</script>

<style>
.header {
  display: flex;
  align-items: center;
  padding: 10px 15px;
  position: relative;
  justify-content: space-between;
  /* background: rgba(0, 0, 0, 0.2); */
}

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

.container {
  height: 90%;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--accent-color) transparent;
  position: relative;
  -ms-overflow-style: -ms-autohiding-scrollbar;
}

.blur-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: 10;

  /* 初始状态 */
  opacity: 0;
  backdrop-filter: blur(5px);
  background-color: rgba(0, 0, 0, 0.45);

  /* 过渡效果 */
  transition: opacity 0.3s ease;

  /* 确保覆盖层可以点击穿透 */
  pointer-events: none;
}
</style>
