<template>
    <Background />
    <MainPanel style="z-index: 1" />
    <Effect style="z-index: 2" :effect="uiStatus.main.effect" />
    <FastLoader style="z-index: 3" />
    <Loader style="z-index: 4" />
    <AudioPlayer :source="uiStatus.main.audio" />
</template>

<script setup lang="ts">
import { uiStatus } from "./api/store";
import { FastLoader, Loader } from "./components/loaders";
import { Background, Effect } from "./components/views";
import AudioPlayer from "./components/views/AudioPlayer.vue";
import MainPanel from "./components/views/MainPanel.vue";
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
    --accent-color: #79d9ff;
    --menu-max-width: 1100px;
}

body,
html {
    margin: 0;
    padding: 0;
    width: 100%;
    height: 100%;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
    overflow: hidden;
    background: transparent;
}

.glass-effect {
    background-color: rgba(255, 255, 255, 0.1);
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    border: 1px solid rgba(255, 255, 255, 0.125);
    box-shadow:
        0 8px 32px rgba(0, 0, 0, 0.1),
        inset 0 1px 1px rgba(255, 255, 255, 0.1);
    transition:
        border-color 0.2s,
        box-shadow 0.2s;
}

*:focus-visible {
    /* 只在Tab控制时，添加此focus */
    outline: 3px solid #79d9ff;
    background-color: #79d9ff80;
}
button {
    user-select: none;
}
</style>
