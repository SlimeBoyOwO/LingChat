<template>
  <button :class="type" :disabled="disabled" @click="$emit('click')">
    <Icon v-if="icon" :icon="icon" :size="icon_size"></Icon>
    <slot></slot>
  </button>
</template>

<script setup lang="ts">
// 导入外部模块
import Icon from "./Icon.vue";
import type { IconType } from "./Icon.vue";

// 定义组件属性
interface ButtonProps {
  type?:
    | "big"
    | "menu"
    | "nav"
    | "select"
    | "delete"
    | "add"
    | "save"
    | "start"
    | "close";
  disabled?: boolean;
  icon?: IconType;
  icon_size?: number;
}
const props = defineProps<ButtonProps>();

// 定义组件事件
const emit = defineEmits(["click"]);
</script>

<style scoped>
button {
  border: none;
  outline: none;
  cursor: pointer;
  font-weight: 500;
  padding: 8px 16px;
  border-radius: 4px;
  transition: all 0.2s ease;
}

/* 禁用状态 */
.base-button--disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.menu {
  background: transparent; /* 去除背景 */
  color: white;
  border: none; /* 去除边框 */
  padding: 15px;
  margin: 10px 0;
  border-radius: 12px;
  /* 使用clamp()实现响应式字体大小 */
  /* 最小32px, 根据视口宽度的4%缩放, 最大72px */
  font-size: clamp(32px, 4vw, 72px);
  font-weight: normal; /* 字体加粗 */
  font-family: "Maoken Assorted Sans", -apple-system, BlinkMacSystemFont,
    "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif; /* 应用自定义字体，并提供备用字体 */
  cursor: pointer;
  transition: color 0.3s, text-shadow 0.3s; /* 平滑过渡 */
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.5); /* 加一点文字阴影以保证清晰度 */
  text-align: left; /* 文字左对齐 */
}

.menu:hover {
  color: #f0f0f0;
  text-shadow: 0 0 10px rgba(255, 255, 255, 0.8); /* 悬停时发光效果 */
  transform: none; /* 移除之前的缩放效果 */
}

/* 开始按钮样式 */
.start {
  background-color: #4caf50;
  color: white;
}

.start:hover {
  background-color: #45a049;
}

/* 关闭按钮样式 */
.close {
  background-color: #f44336;
  color: white;
}

.close:hover {
  background-color: #d32f2f;
}

.nav {
  color: white; /* 改为白色 */
  background: none;
  text-align: center;
  padding: 10px 15px;
  border-radius: 8px;
  border: none;
  cursor: pointer;
  margin: 0 5px;
  font-size: 16px;
  font-weight: bold;
  position: relative;
  z-index: 1;
  transition: color 0.3s ease, background-color 0.3s ease;
  display: flex;
  align-items: center;
  gap: 8px;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
}

.nav svg {
  width: 18px;
  height: 18px;
  stroke-width: 2.5px;
  flex-shrink: 0;
}

.nav:hover {
  color: var(--accent-color);
}
.nav.active {
  color: var(--accent-color);
  background-color: rgba(255, 255, 255, 0.1); /* 半透明白色背景 */
}

.nav.active:hover {
  color: var(--accent-color);
  background-color: rgba(255, 255, 255, 0.15);
}

.big {
  width: 100%;
  padding: 12px;
  font-size: 16px;
  font-weight: bold;
  border-radius: 8px;
  border: none;
  cursor: pointer;
  background-color: #e9ecef;
  color: #495057;
  transition: all 0.2s ease;
}

.big:hover {
  background-color: var(--accent-color);
  color: white;
  transform: translateY(-2px);
  box-shadow: 0 4px 10px rgba(121, 217, 255, 0.4);
}

.select {
  align-self: flex-end; /* 按钮靠右 */
  background-color: #ccc;
  color: #666;
  border: none;
  padding: 8px 15px;
  border-radius: 20px;
  cursor: pointer;
  transition: all 0.2s ease;
  font-size: 13px;
  font-weight: 500;
}
.select.selected {
  background-color: var(--accent-color);
  color: white;
}
.select:not(.selected):hover {
  background-color: #555;
  color: white;
}
</style>
