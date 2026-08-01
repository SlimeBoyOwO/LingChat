<template>
  <MenuPage>
    <MenuItem :title="$t('settings.log.title')" size="large">
      <template #header>
        <ScrollText :size="20" />
      </template>

      <LogConsole />
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { MenuPage, MenuItem } from '../../ui'
import { ScrollText } from 'lucide-vue-next'
import LogConsole from './LogConsole.vue'
</script>
