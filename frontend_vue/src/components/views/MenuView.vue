<template>
  <Transition name="main-menu-animation" :duration="300">
    <div class="menu-container" v-if="uiStatus.currentPage === 'MenuView'">
      <div class="main-menu">
        <template v-for="item in menuItems">
          <button
            class="menu-item"
            v-if="item.visibility.value"
            :key="item.order"
            @click="item.action"
          >
            {{ item.label }}
          </button>
        </template>
      </div>
    </div>
  </Transition>
  <img
    src="../../assets/images/LingChatLogo.png"
    alt="LingChatLogo"
    class="logo"
  />
</template>
<script setup lang="ts">
import { ref, computed } from "vue";
import { uiStatus, userData } from "../../api/store";
const menuItems = [
  {
    order: 0,
    label: "继续游戏",
    action: continueGame,
    visibility: computed(() => {
      return (
        userData.value.isInitialized &&
        userData.value.current_card.save.isLoadAvailable
      );
    }),
  },
  { order: 1, label: "开始游戏", action: newGame, visibility: ref(true) },
  { order: 2, label: "存档", action: openSave, visibility: ref(true) },
  { order: 3, label: "设置", action: openSettings, visibility: ref(true) },
  { order: 4, label: "退出游戏", action: quitGame, visibility: ref(true) },
];
function continueGame() {}
function newGame() {}
function openSave() {
  uiStatus.value.switchPage("save").beginLoading(true);
}
function openSettings() {
  uiStatus.value.switchPage("settings").beginLoading(true);
}
function quitGame() {
  window.close();
}
</script>
<style></style>
