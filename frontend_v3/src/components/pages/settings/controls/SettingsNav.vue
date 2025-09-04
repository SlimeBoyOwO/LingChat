<template>
    <div class="settings-nav" :data-level="level">
        <img id="settings-logo" src="/src/assets/images/LingChatLogo.png" alt="Logo" v-if="level === 1" />
        <IconButton
            v-for="(single, title) in source"
            :key="title"
            :id="(title as string)!"
            :icon="single.icon"
            @click="current = (title as string)!"
            :text="(title as string)!"
        />
        <IconButton
            v-if="level === 1"
            id="settings-close-button"
            icon="close"
            text="Close"
            @click="uiStatus.main.back()"
        />
    </div>
    <SettingsNav
        v-if="level === 1 && isSettingNode2(source[current].content)"
        :level="2"
        :source="(source[current].content as SettingNode2)!"
    />
    <SettingContent v-else :controls="(source[current].content as SettingPage[])!" :title="current" />
</template>
<script setup lang="ts">
import { ref } from "vue";

import { uiStatus } from "../../../../api/store";
import { SettingNode1, SettingNode2, SettingPage } from "../../../../api/types/settings";
import IconButton from "../../../controls/IconButton.vue";
import SettingContent from "./SettingContent.vue";

function isSettingNode2(source: SettingNode2 | SettingPage[]): source is SettingNode2 {
    return !Array.isArray(source);
}
const { level, source } = defineProps<{ level: number; source: SettingNode1 | SettingNode2 }>();
const current = ref(getCurrent());
function getCurrent() {
    const page = uiStatus.value.settings.read<{ page: string }>()?.page;
    if (page === undefined) return Object.keys(source)[0];
    if (source[page] === undefined) return Object.keys(source)[0];
    return page;
}
</script>
<style>
.settings-nav {
    display: flex;
}

.settings-nav[data-level="1"] {
    width: 100vw;
    height: 80px;
    padding-right: 100px;
    flex-direction: row;
    align-items: center;
    justify-content: center;
    border-bottom: 5px solid var(--accent-color);
}

.settings-nav[data-level="2"] {
    top: 80px;
    width: 15%;
    height: 100%;
    min-width: 200px;
    padding: 20px;
    border-right: 5px solid lightgreen;
    flex-direction: column;
    align-items: center;
    justify-content: start;
}

.settings-nav > .icon-button {
    color: white;
    background: none;
    text-align: center;
    padding: 10px 15px;
    border-radius: 8px;
    border: none;
    cursor: pointer;
    margin: 0 5px;
    font-size: 22px;
    font-weight: bold;
    position: relative;
    transition:
        color 0.3s ease,
        background-color 0.3s ease;
    display: flex;
    align-items: center;
    gap: 8px;
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
}

.settings-nav > .icon-button:hover {
    color: var(--accent-color);
}

.settings-nav > .icon-button:active {
    color: var(--accent-color);
    background-color: rgba(255, 255, 255, 0.1);
}

.settings-nav > .icon-button:active:hover {
    color: var(--accent-color);
    background-color: rgba(255, 255, 255, 0.15);
}

.settings-nav[data-level="1"] > #settings-close-button {
    position: fixed;
    top: 0;
    right: 0;
    margin: 20px;
}
.settings-nav[data-level="1"] > #settings-logo {
    position: fixed;
    top: 0;
    left: 0;
    margin: 10px;
    height: 60px;
}
</style>
