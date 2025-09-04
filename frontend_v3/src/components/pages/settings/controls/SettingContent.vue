<template>
    <div class="settings-content" :id="title">
        <div v-for="(control, index) in controls" :key="index">
            <h2>{{ control.bindings.title }}</h2>
            <hr />
            <component
                :is="control.element"
                v-bind="control.bindings"
                v-if="control.bindings.show"
            >
            </component>
        </div>
    </div>
</template>
<script setup lang="ts">
import { SettingPage } from "../../../../api/types/settings";
// eslint-disable-next-line @typescript-eslint/no-unused-vars
import {Icon,IconButton,Input,Slider,Text,Toggle} from "../../../controls";
const { title, controls } = defineProps<{ title: string; controls: SettingPage[] }>();
</script>
<style scoped>
.settings-content {
    position: relative;
    width: 100%;
    height: 100%;
    padding: 20px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: start;
}
.settings-content > div {
    width: 80%;
    max-width: var(--menu-max-width);
    height: fit-content;
    min-height: 100px;
    border-radius: 20px;
    padding: 15px;
    display:flex;
    flex-direction: column;
    justify-content: space-between;
    overflow-x: hidden;
    overflow-y: scroll;
    scrollbar-color: transparent transparent;
    background-color: rgba(255, 255, 255, 0.1);
}
</style>
