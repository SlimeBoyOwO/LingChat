<template>
  <MenuPage>
    <MenuItem title="存档"> </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { MenuPage } from "../../ui";
import { MenuItem } from "../../ui";

import { useGameStore } from "../../../stores/modules/game";

const gameStore = useGameStore();

// gameStore.getDialogHistory();
</script>

<style scoped>
/* 样式部分 */
</style>
