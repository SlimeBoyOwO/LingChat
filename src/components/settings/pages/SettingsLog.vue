<template>
  <MenuPage>
    <MenuItem data-setting-anchor="log-top" :title="$t('settings.log.title')" size="large">
      <template #header>
        <ScrollText :size="20" />
      </template>

      <LogConsole />
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { MenuPage, MenuItem } from "../../ui";
import { ScrollText } from "@lucide/vue";
import LogConsole from "./LogConsole.vue";
import { focusSettingAnchor, useSettingsJumpConsumer } from "../search/jumpHighlight";

useSettingsJumpConsumer(
  (jump) => jump.settingsTab === "log",
  async (jump) => {
    await focusSettingAnchor(jump.anchor || "log-top", {
      crossTab: jump.crossTab,
      fallbackAnchor: jump.fallbackAnchor || "log-top",
    });
  },
);
</script>
