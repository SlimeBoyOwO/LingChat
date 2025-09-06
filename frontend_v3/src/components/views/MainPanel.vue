<template>
    <div class="main-panel">
        <MenuView />
        <ChatView />
        <Credits />
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
import Credits from "./Credits.vue";

onMounted(() => uiStatus.value.main.switchPage(PAGES.MAIN.MENU));
</script>

<style>
.main-panel {
    width: 100%;
    height: 100%;
    position: relative;
}

.menu-container {
    width: 100%;
    height: 100%;
    display: flex;
    justify-content: flex-start;
    align-items: center;
}

.main-menu {
    display: flex;
    flex-direction: column;
    padding: 20px;
    margin-left: 10vw;
    position: relative;
}

.logo {
    position: absolute;
    top: 5vh;
    left: auto;
    right: 5vw;
    height: 40vh;
    width: auto;
    max-width: 40vw;
    filter: drop-shadow(0 4px 8px rgba(0, 0, 0, 0.3));
}

.menu-item {
    background: transparent;
    color: white;
    border: none;
    padding: 15px;
    margin: 10px 0;
    border-radius: 12px;
    font-size: clamp(32px, 4vw, 72px);
    font-weight: normal;
    font-family:
        "Maoken Assorted Sans",
        -apple-system,
        BlinkMacSystemFont,
        "Segoe UI",
        Roboto,
        "Helvetica Neue",
        Arial,
        sans-serif;
    cursor: pointer;
    transition:
        color 0.3s,
        text-shadow 0.3s;
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
    text-align: justify;
    text-align-last: justify;
}

.menu-item:hover {
    color: #f0f0f0;
    text-shadow: 0 0 10px rgba(255, 255, 255, 0.8);
    transform: none;
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
</style>
