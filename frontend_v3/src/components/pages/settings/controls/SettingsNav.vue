<template>
    <div class="settings-nav" :data-level="level">
        <div v-for="(single, title) in source" :key="title" :id="(title as string)!" @click="current = source[title]">
            <img :src="single.icon" />
            <span>{{ i18n(title as string) }}</span>
        </div>
    </div>
    <SettingsNav
        v-if="level === 1 && isSettingNode2(current.content)"
        :level="2"
        :source="(current.content as SettingNode2)!"
    />
    <SettingContent v-else-if="level === 2" :controls="(current.content as SettingPage[])!" />
    <SettingContent v-else :controls="(current.content as SettingPage[])!" />
</template>
<script setup lang="ts">
import { defineProps, shallowRef } from "vue";

import { i18n, uiStatus } from "../../../../api/store";
import SettingContent from "./SettingContent.vue";
import { SettingNode1, SettingNode2, SettingPage } from "../../../../api/types/settings";

function isSettingNode2(source: SettingNode2 | SettingPage[]): source is SettingNode2 {
    return !Array.isArray(source);
}
const { level, source } = defineProps<{ level: number; source: SettingNode1 | SettingNode2 }>();
const current = shallowRef(getCurrent());
function getCurrent() {
    const page = uiStatus.value.settings.read<{ page: string }>()?.page;
    if (page === undefined) return source[Object.keys(source)[0]];
    const content = source[page];
    if (content === undefined) return source[Object.keys(source)[0]];
    return content;
}
</script>
<style>
.settings-nav {
    position: fixed;
    display: flex;
    align-items: center;
    justify-content: start;
}

.settings-nav[data-level="1"] {
    width: 100vw;
    height: fit-content;
    min-height: 80px;
    flex-direction: row;
    background-color: lightblue;
}

.settings-nav[data-level="2"] {
    width: 20%;
    min-width: 300px;
    flex-direction: column;
    background-color: lightgreen;
}
</style>
