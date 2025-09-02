<template>
    <div :class="['main-panel', { blur: uiStatus.main.currentPage !== PAGES.MAIN.MENU }]">
        <MenuView />
        <ChatView />
        <SettingsView />
    </div>
</template>

<script setup lang="ts">
import { onMounted } from "vue";

import { MenuView } from ".";
import { PAGES } from "../../api/consts";
import { uiStatus } from "../../api/store";
import { ChatView } from "../pages/chat";
import { SettingsView } from "../pages/settings";

onMounted(() => uiStatus.value.main.switchPage(PAGES.MAIN.MENU));
</script>

<style>
.main-panel {
    width: 100%;
    height: 100%;
    position: relative;
}

.main-panel.blur {
    backdrop-filter: blur(8px);
    transition: backdrop-filter 0.5s ease-in;
}

/* 主容器，用于设置背景和布局 */
.menu-container {
    width: 100%;
    height: 100%;
    display: flex;
    justify-content: flex-start;
    /* 将主菜单推到左边 */
    align-items: center;
}

/* 主菜单 */
.main-menu {
    display: flex;
    flex-direction: column;
    /* align-items: center; */
    padding: 20px;
    margin-left: 10vw;
    /* 距离左侧边缘 10% 视口宽度 */
    position: relative;
    /* 确保菜单显示在背景之上 */
    z-index: 1;
}

.logo {
    position: absolute;
    top: 5vh;
    /* 距离顶部 5% 视口高度 */
    left: auto;
    right: 5vw;
    /* 距离右侧 5% 视口宽度 */
    height: 40vh;
    /* 高度为视口高度的 40% */
    width: auto;
    /* 宽度自动，保持比例 */
    max-width: 40vw;
    /* 最大宽度不超过视口宽度的 40%，防止过宽 */
    filter: drop-shadow(0 4px 8px rgba(0, 0, 0, 0.3));
    /* 添加一点阴影使其更突出 */
    z-index: 1;
    /* 确保 logo 显示在背景之上 */
}

.menu-item {
    background: transparent;
    /* 去除背景 */
    color: white;
    border: none;
    /* 去除边框 */
    padding: 15px;
    margin: 10px 0;
    border-radius: 12px;
    /* 使用 clamp() 实现响应式字体大小 */
    /* 最小 32px, 根据视口宽度的 4% 缩放，最大 72px */
    font-size: clamp(32px, 4vw, 72px);
    font-weight: normal;
    /* 字体加粗 */
    font-family:
        "Maoken Assorted Sans",
        -apple-system,
        BlinkMacSystemFont,
        "Segoe UI",
        Roboto,
        "Helvetica Neue",
        Arial,
        sans-serif;
    /* 应用自定义字体，并提供备用字体 */
    cursor: pointer;
    transition:
        color 0.3s,
        text-shadow 0.3s;
    /* 平滑过渡 */
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
    /* 加一点文字阴影以保证清晰度 */
    text-align: justify;
    text-align-last: justify;
    /* 文字两端对齐 */
}

.menu-item:hover {
    color: #f0f0f0;
    text-shadow: 0 0 10px rgba(255, 255, 255, 0.8);
    /* 悬停时发光效果 */
    transform: none;
    /* 移除之前的缩放效果 */
}

.main-menu-animation-enter-active .main-menu,
.main-menu-animation-leave-active .main-menu,
.main-menu-animation-enter-active .logo,
.main-menu-animation-leave-active .logo {
    transition: all 0.3s ease-in-out;
}

.main-menu-animation-enter-from .main-menu,
.main-menu-animation-leave-to .main-menu {
    transform: translateX(-120%);
    opacity: 0;
}

.main-menu-animation-enter-from .logo,
.main-menu-animation-leave-active .logo {
    transform: translateX(120%);
    opacity: 0;
}

/* 特定面板的显示规则 */
body.panel-active.show-settings .settings-panel {
    transform: translateX(0);
    opacity: 1;
}

body.panel-active.show-game-screen .game-screen-panel {
    transform: translateX(0);
    opacity: 1;
    pointer-events: auto;
}

body.panel-active.show-load-save .load-save-panel {
    transform: translateX(0);
    opacity: 1;
    pointer-events: auto;
}
</style>
