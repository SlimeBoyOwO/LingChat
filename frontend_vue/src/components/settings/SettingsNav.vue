<template>
  <div class="nav-container">
    <img
      src="@/assets/images/LingChatLogo.png"
      alt="Logo"
      class="settings-logo"
    />
    <nav>
      <div ref="indicator" class="nav-indicator"></div>
      <Button
        ref="characterBtn"
        type="nav"
        icon="character"
        @click="() => switchTab('character', 'characterBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'character' }"
        >角色</Button
      >
      <Button
        ref="textBtn"
        type="nav"
        icon="text"
        @click="() => switchTab('text', 'textBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'text' }"
        >文本</Button
      >
      <Button
        ref="backgroundBtn"
        type="nav"
        icon="background"
        @click="() => switchTab('background', 'backgroundBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'background' }"
        >背景</Button
      >
      <Button
        ref="soundBtn"
        type="nav"
        icon="sound"
        @click="() => switchTab('sound', 'soundBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'sound' }"
        >声音</Button
      >
      <Button
        ref="historyBtn"
        type="nav"
        icon="history"
        @click="() => switchTab('history', 'historyBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'history' }"
        >对话历史</Button
      >
      <Button
        ref="advanceBtn"
        type="nav"
        icon="advance"
        @click="() => switchTab('advance', 'advanceBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'advance' }"
        >高级设置</Button
      >
    </nav>
    <Icon
      icon="close"
      class="close-button"
      :size="40"
      @click="closeSettings"
    ></Icon>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from "vue";
import { useUIStore } from "../../stores/modules/ui/ui";
import { Button } from "../base";
import Icon from "../base/widget/Icon.vue";

const uiStore = useUIStore();
const indicator = ref<HTMLElement | null>(null);

// 定义按钮ref的类型
type ButtonRef = InstanceType<typeof Button>;

// 使用更宽松的类型定义
const characterBtn = ref<ButtonRef | null>(null);
const textBtn = ref<ButtonRef | null>(null);
const backgroundBtn = ref<ButtonRef | null>(null);
const soundBtn = ref<ButtonRef | null>(null);
const historyBtn = ref<ButtonRef | null>(null);
const advanceBtn = ref<ButtonRef | null>(null);

// 统一处理标签切换
const switchTab = (tabName: string, refName: string) => {
  uiStore.setSettingsTab(tabName);
  const buttonRef = {
    characterBtn,
    textBtn,
    backgroundBtn,
    soundBtn,
    historyBtn,
    advanceBtn,
  }[refName];

  if (buttonRef?.value?.$el) {
    moveIndicator(buttonRef.value.$el);
  }
};

// 移动指示器
const moveIndicator = (target: HTMLElement) => {
  if (!indicator.value || !target) return;

  indicator.value.style.left = `${target.offsetLeft}px`;
  indicator.value.style.width = `${target.offsetWidth}px`;
};

// 初始化指示器位置
const initIndicator = () => {
  const activeTab = uiStore.currentSettingsTab;
  let activeButton = null;

  switch (activeTab) {
    case "character":
      activeButton = characterBtn.value;
      break;
    case "text":
      activeButton = textBtn.value;
      break;
    case "background":
      activeButton = backgroundBtn.value;
      break;
    case "sound":
      activeButton = soundBtn.value;
      break;
    case "history":
      activeButton = historyBtn.value;
      break;
    case "advance":
      activeButton = advanceBtn.value;
      break;
  }

  if (activeButton?.$el) {
    moveIndicator(activeButton.$el);
  }
};

// 组件挂载后初始化指示器
onMounted(() => {
  initIndicator();
});

// 监听当前标签变化
watch(
  () => uiStore.currentSettingsTab,
  () => {
    initIndicator();
  }
);

const closeSettings = () => {
  uiStore.toggleSettings(false);
};
</script>

<style>
/* 样式保持不变 */
.nav-container {
  position: relative; /* 为绝对定位的子元素建立参照 */
  display: flex;
  align-items: center;
  justify-content: space-between; /* 均匀分布元素 */
  padding: 0 20px; /* 添加一些内边距 */
  height: 100%; /* 或其他适当高度 */
  width: 100%;
}

.settings-logo {
  width: 80px;
  filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.2));
  z-index: 2; /* 确保logo在最上层 */
}

img {
  overflow-clip-margin: content-box;
  overflow: clip;
}

nav {
  position: relative;
  left: auto;
  transform: none;
  display: flex;
  flex-direction: row;
  padding: 0;
  height: 100%;
  align-items: center;
  flex-grow: 1; /* 允许nav占据可用空间 */
  justify-content: center; /* 居中导航按钮 */
}

.nav-indicator {
  position: absolute;
  bottom: 0;
  left: 0;
  width: 0;
  height: 4px;
  background-color: var(--accent-color);
  border-radius: 2px;
  z-index: 1;
  transition: left 0.3s ease-in-out, width 0.3s ease-in-out;
  box-shadow: 0 0 10px rgba(121, 217, 255, 0.4);
}

.close-button {
  background: transparent;
  border: none;
  padding: 6px;
  border-radius: 50%;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
  transition: all 0.3s ease-in-out;
}

.close-button:hover {
  color: var(--accent-color);
  background-color: rgba(255, 255, 255, 0.1);
  transform: rotate(90deg);
}
</style>
