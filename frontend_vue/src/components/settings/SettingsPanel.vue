<template>
  <div
    class="blur-overlay"
    v-if="shouldShowOverlay"
    :style="{ opacity: overlayOpacity }"
  ></div>
  <div class="settings-panel" v-show="uiStore.showSettings">
    <div class="header">
      <SettingsNav
        ref="settingsNavRef"
        @remove-more-menu-from-a="onAddFromA"
      />
    </div>
    <div class="container">
      <SettingsSave v-show="uiStore.currentSettingsTab === 'save'" />
      <SettingsText v-show="uiStore.currentSettingsTab === 'text'" />
      <SettingsSound v-show="uiStore.currentSettingsTab === 'sound'" />
      <SettingsAdvance
        ref="settingsAdvanceRef"
        v-show="uiStore.currentSettingsTab === 'advance'" 
        @remove-more-menu-from-b="onAddFromB"
      />
      <SettingsHistory v-show="uiStore.currentSettingsTab === 'history'" />
      <SettingsSchedule v-show="uiStore.currentSettingsTab === 'schedule'" />
      <SettingsCharacter v-show="uiStore.currentSettingsTab === 'character'" />
      <SettingsBackground
        v-show="uiStore.currentSettingsTab === 'background'"
      />
      <SettingsUpdate v-show="uiStore.currentSettingsTab === 'update'" />
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
  SettingsUpdate,
} from "./pages";
import SettingsNav from "./SettingsNav.vue";
import { useUIStore } from "../../stores/modules/ui/ui";
import { ref, watch } from "vue";

const uiStore = useUIStore();

// 获取 A 组件和 B 组件的 Ref 实例
const settingsNavRef = ref<InstanceType<typeof SettingsNav> | null>(null);
const settingsAdvanceRef = ref<InstanceType<typeof SettingsAdvance> | null>(null);

// 添加延迟状态
const shouldShowOverlay = ref(false);
const overlayOpacity = ref(0);

watch(
  () => uiStore.showSettings,
  (newVal) => {
    if (newVal) {
      // 显示时：立即显示元素，然后延迟改变透明度
      shouldShowOverlay.value = true;
      setTimeout(() => {
        overlayOpacity.value = 1;
      }, 10); // 使用很小的延迟确保浏览器有机会渲染
    } else {
      // 隐藏时：先改变透明度，然后延迟隐藏元素
      overlayOpacity.value = 0;
      setTimeout(() => {
        shouldShowOverlay.value = false;
      }, 100); // 匹配你的动画持续时间
    }
  },
  { immediate: true }
);

// 2. 定义事件处理函数
// 当 A 组件发来 "remove-more-menu-from-a" 事件时
const onAddFromA = () => {
  // console.log('父组件收到 A 的添加事件，准备通知 B 组件');
  // 调用 B 组件实例上暴露的 removeMoreMenu 方法
  settingsAdvanceRef.value?.addMoreMenu();
};

// 当 B 组件发来 "remove-more-menu-from-b" 事件时
const onAddFromB = () => {
  // console.log('父组件收到 B 的添加事件，准备通知 A 组件');
  // 调用 A 组件实例上暴露的 removeMoreMenu 方法
  settingsNavRef.value?.addMoreMenu();
};
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
