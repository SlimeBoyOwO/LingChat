<template>
    <Transition @afterEnter="mounted">
        <div class="settings-panel" v-if="uiStatus.main.currentPage === PAGES.MAIN.SETTINGS">
            <SettingsNav :level="1" :source="settings_structure" />
        </div>
    </Transition>
</template>

<script setup lang="ts">
import { PAGES } from "../../../api/consts";
import { settings_structure, uiStatus } from "../../../api/store";
import SettingsNav from "./controls/SettingsNav.vue";

function mounted() {
    uiStatus.value.main.background.isBlur = true;
    uiStatus.value.main.endLoading();
}
</script>

<style scoped>
.settings-panel {
    width: 100vw;
    height: 100vh;
    backdrop-filter: blur(5px);
    background-color: rgba(0, 0, 0, 0.45);
    z-index: 1000;
    color: #333;
}

#settings-nav {
    position: relative;
    width: fit-content;
    height: 100%;
}

#settings-content {
    position: relative;
    width: auto;
    height: 100%;
}
</style>
