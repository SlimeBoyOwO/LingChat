<template>
    <Transition @before-enter="beforeMount" @after-enter="mounted" name="main-menu-animation" :duration="300">
        <div class="menu-container" v-if="uiStatus.main.currentPage === 'MenuView'">
            <div class="main-menu">
                <template v-for="item in menuItems">
                    <button class="menu-item" v-if="item.visibility.value" :key="item.order" @click="item.action">
                        {{ item.label }}
                    </button>
                </template>
            </div>
            <img
                src="/src/assets/images/LingChatLogo.png"
                alt="LingChatLogo"
                class="logo"
                :style="{ height: 'auto' }"
            />
        </div>
    </Transition>
</template>
<script setup lang="ts">
import { computed, onBeforeMount, onMounted, ref } from "vue";

import { i18n, uiStatus, userData } from "../../api/store";

onBeforeMount(beforeMount);
onMounted(mounted);
function beforeMount() {
    uiStatus.value.main.background_image = "/src/assets/images/background.png";
}
function mounted() {
    uiStatus.value.main.endLoading();
}

const menuItems = [
    {
        order: 0,
        label: i18n.value("main.continueGame"),
        action: continueGame,
        visibility: computed(() => (userData.value.current_card?.save.count ?? 0) > 0)
    },
    { order: 1, label: i18n.value("main.newGame"), action: newGame, visibility: ref(true) },
    { order: 2, label: i18n.value("main.saveGame"), action: openSave, visibility: ref(true) },
    { order: 3, label: i18n.value("main.settings"), action: openSettings, visibility: ref(true) },
    { order: 4, label: i18n.value("main.quitGame"), action: quitGame, visibility: ref(true) }
];
function continueGame() {}
function newGame() {
    uiStatus.value.main.switchPage("ChatView");
    console.log(uiStatus.value.main.__nav_stack);
    console.log(uiStatus.value.main.currentPage);
}
function openSave() {
    uiStatus.value.settings.switchPage("save").with({ status: "load" });
    uiStatus.value.main.switchPage("Settings").beginLoading(true);
}
function openSettings() {
    uiStatus.value.main.switchPage("Settings").beginLoading(true);
}
function quitGame() {
    window.close();
}
</script>
<style />
