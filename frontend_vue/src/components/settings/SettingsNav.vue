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
        ><p class="button-text">角色</p></Button
      >
      <Button
        ref="textBtn"
        type="nav"
        icon="text"
        @click="() => switchTab('text', 'textBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'text' }"
        ><p class="button-text">通用</p></Button
      >
      <Button
        ref="backgroundBtn"
        type="nav"
        icon="background"
        @click="() => switchTab('background', 'backgroundBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'background' }"
        ><p class="button-text">背景</p></Button
      >
      <Button
        ref="soundBtn"
        type="nav"
        icon="sound"
        @click="() => switchTab('sound', 'soundBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'sound' }"
        ><p class="button-text">声音</p></Button
      >
      <Button
        ref="historyBtn"
        type="nav"
        icon="history"
        @click="() => switchTab('history', 'historyBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'history' }"
        ><p class="button-text">对话历史</p></Button
      >
      <Button
        ref="saveBtn"
        type="nav"
        icon="save"
        @click="() => switchTab('save', 'saveBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'save' }"
        ><p class="button-text">存档</p></Button
      >
      <Button
        ref="scheduleBtn"
        type="nav"
        icon="schedule"
        @click="() => switchTab('schedule', 'scheduleBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'schedule' }"
        ><p class="button-text">日程</p></Button
      >
      <Button
        ref="advanceBtn"
        type="nav"
        icon="advance"
        @click="() => switchTab('advance', 'advanceBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'advance' }"
        ><p class="button-text">高级设置</p></Button
      >
      <Button
        ref="updateBtn"
        type="nav"
        icon="update"
        @click="() => switchTab('update', 'updateBtn')"
        :class="{ active: uiStore.currentSettingsTab === 'update' }"
        ><p class="button-text">检查更新</p></Button
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
const saveBtn = ref<ButtonRef | null>(null);
const advanceBtn = ref<ButtonRef | null>(null);
const scheduleBtn = ref<ButtonRef | null>(null);
const updateBtn = ref<ButtonRef | null>(null);

// 设置可重设的值（使用 ref 存储，确保响应式或跨函数访问）
const oldRefName = ref("textBtn");

// 提取：根据 refName 获取按钮并移动指示器
const handleIndicatorMove = (currentRefName: string) => {
  const buttonRef = {
    characterBtn,
    textBtn,
    backgroundBtn,
    soundBtn,
    historyBtn,
    saveBtn,
    advanceBtn,
    scheduleBtn,
    updateBtn,
  }[currentRefName];

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

// 统一处理标签切换
const switchTab = (tabName: string, refName: string) => {
  // 记录当前 refName 到 oldRefName
  oldRefName.value = refName;
  uiStore.setSettingsTab(tabName);
  
  if (!indicator.value) return;

  // 1. 设置过渡动画
  indicator.value.style.transition = 'left 0.3s cubic-bezier(0.18, 0.89, 0.32, 1), width 0.3s cubic-bezier(0.18, 0.89, 0.32, 1)';
  
  // 2. 触发动画（移动指示器）
  handleIndicatorMove(refName);

  // 3. 使用 setTimeout 延迟执行 unset
  //    延迟时间设置为 400ms，略长于动画时长 300ms，确保动画完全结束
  setTimeout(() => {
    if (indicator.value) { // 再次检查 indicator 是否存在，避免组件卸载后报错
      indicator.value.style.transition = 'unset';
    }
  }, 400); // 延迟 400 毫秒
};

// 屏幕宽度变化监测器
const setupResizeObserver = () => {
  const resizeObserver = new ResizeObserver(entries => {
    // 宽度变化时，从 oldRefName 提取 refName 并执行逻辑
    if (oldRefName.value) {
      handleIndicatorMove(oldRefName.value);
    }
  });

  // 监听整个窗口的大小变化
  resizeObserver.observe(window.document.body);

};

// 初始化监听器
setupResizeObserver();

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
    case "save":
      activeButton = saveBtn.value;
      break;
    case "advance":
      activeButton = advanceBtn.value;
      break;
    case "schedule":
      activeButton = scheduleBtn.value;
      break;
    case "update":
      activeButton = updateBtn.value;
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
  overflow-x: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--accent-color) transparent;
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

/* 在 html 的 width 低于 1220px 将会隐藏文字，避免文本换行 */
@media (max-width: 1220px) {
  .button-text {
    display: none;
  }
}

/* 新增媒体查询，用于适配窄屏幕 */
@media (max-width: 768px) {
  /* 隐藏 Logo */
  .settings-logo {
    display: none;
  }

  /* 调整导航栏布局 */
  nav {
    justify-content: space-around; /* 让仅有图标的按钮均匀分布 */
    flex-grow: 1; /* 占据全部可用空间 */
  }

  .nav-container {
    padding: 0 10px; /* 在手机上可以减少一些左右边距 */
  }
}
</style>
