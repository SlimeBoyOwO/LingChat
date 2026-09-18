<template>
  <router-view />
  <!-- macOS 无边框窗口：顶部拖拽区（配合 Overlay 红绿灯）。仅在 macOS 主窗口挂载，
       其余窗口 / Windows / Linux / 移动端不渲染，避免影响既有拖动与点击。 -->
  <div v-if="isMacOverlayWindow" class="mac-drag-region" data-tauri-drag-region></div>
  <!-- 将光标特效 teleport 到 body，避免 #app 上的整体缩放（transform: scale）导致坐标偏移 -->
  <Teleport to="body">
    <CursorEffects />
  </Teleport>

  <!-- 全局通知组件（直接从 uiStore 读取状态） -->
  <!-- 与桌宠专用通知组件区分开 -->
  <!-- 弹窗类组件仅主窗口挂载：日志等独立窗口复用 App.vue，不重复弹出 -->
  <Notification v-if="isMainWindow && route.path !== '/pet'" />
  <AchievementToast v-if="isMainWindow" />
  <AdventureUnlockNotify v-if="isMainWindow" />
  <AppDialog v-if="isMainWindow" />
</template>

<script setup lang="ts">
import { useRoute } from "vue-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import CursorEffects from "./components/effects/CursorEffects.vue";
import Notification from "./components/ui/Notification.vue";
import AchievementToast from "./components/ui/AchievementToast.vue";
import AdventureUnlockNotify from "./components/ui/AdventureUnlockNotify.vue";
import AppDialog from "./components/ui/AppDialog.vue";
import { useAsrInput } from "./composables/useAsrInput";
import { useCanDeliver } from "./composables/useCanDeliver";
import { useZoom } from "./composables/useZoom";
import { useSedentaryReminder } from "./composables/useSedentaryReminder";
import { useAppBootstrap } from "./composables/app/useAppBootstrap";
import { useCastMirror } from "./composables/app/useCastMirror";
import { useCloseConfirm } from "./composables/app/useCloseConfirm";
import { useDevToolsVisibility } from "./composables/app/useDevToolsVisibility";
import { useFullscreenHotkey } from "./composables/app/useFullscreenHotkey";
import { useGlobalFont } from "./composables/app/useGlobalFont";
import { useMacTitlebar } from "./composables/app/useMacTitlebar";
import { useMobileViewport } from "./composables/app/useMobileViewport";

// ─── 路由 / 窗口标识 ────────────────────────────────────────
const route = useRoute();
// 仅主窗口挂载全局弹窗（通知/成就/对话确认），日志窗口等复用 App.vue 的窗口不弹
const isMainWindow = getCurrentWindow().label === "main";

// ─── 全局单例 composables（仅在此处调用一次以激活）──────────
// 激活主动对话投放条件上报
useCanDeliver();
// 激活 Ctrl+滚轮 UI 全局缩放
useZoom();
// 久坐提醒
useSedentaryReminder();
// ASR 全局初始化（仅主窗口一次）：auto_listen 能量监测门控 + 事件监听。
// useAsrInput 状态是模块级单例，GameDialog / ChatInput（桌宠）的 mic 按钮
// 与这里共享同一会话。
if (isMainWindow) {
  useAsrInput();
}

// ─── 应用外壳关注点（各 composable 头部有详细说明）───────────
// 注意：调用顺序即 onMounted 注册顺序，启动初始化需先于移动端视口与关闭确认。
useAppBootstrap();
useMobileViewport();
useCloseConfirm();

useGlobalFont();
useDevToolsVisibility();
useCastMirror();
useFullscreenHotkey();

// macOS 无边框标题栏顶部安全区；返回的 isMacOverlayWindow 供模板决定是否渲染拖拽区
const { isMacOverlayWindow } = useMacTitlebar();
</script>

<style>
:root {
  /*全局变量*/
  --accent-color: #79d9ff;
  --menu-max-width: 1100px;
  --menu-max-width-half: 550px;
  /* 一个生动的天蓝色，可以根据你的品牌调整 */
}

/* 拖拽区覆盖整个窗口顶部，让无边框窗口（Overlay 标题栏）仍可用鼠标拖动。
   macOS Overlay 下红绿灯悬浮于左上角，拖拽区避开红绿灯区域，防止误触窗口按钮。 */
.mac-drag-region {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  height: 20px;
  /* 红绿灯(约 70px 宽)占据左上角，拖拽区从左 80px 起，避免抢走窗口按钮的点击 */
  margin-left: 80px;
  z-index: 60;
  cursor: default;
  -webkit-app-region: drag;
  pointer-events: auto;
}

/* 全局样式和字体 */
body,
html {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: transparent;
}

#app {
  /* 视口口径统一为动态视口（dvw/dvh，iOS 全屏态下 dvw 横屏自动排除左右安全区、dvh 竖屏含上下安全区）：
     #app 铺满整个视觉视口（含状态栏/Home 指示器区域），使各屏壁纸全出血显示；
     安全区内缩由各边缘元素通过 env(safe-area-inset-*)（桌面/Android 桌面为 0px，零回归）自行处理——
     已在全局提供 --safe-area-inset-* 变量与 .pt-safe/.pb-safe 工具类（见 base.css）。 */
  position: fixed;
  top: 0;
  left: 0;
  width: 100dvw;
  height: 100dvh;
}
</style>
