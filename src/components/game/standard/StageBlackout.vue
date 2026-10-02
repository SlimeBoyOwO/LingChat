<template>
  <!-- Solid stage blackout: hides background/roles while dialogue and menu remain readable. -->
  <div
    v-if="enabled"
    class="pointer-events-none absolute inset-0 bg-black"
    style="z-index: 2"
  />
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useUIStore } from '../../../stores/modules/ui/ui'

const uiStore = useUIStore()
const enabled = computed(() =>
  uiStore.currentBackgroundEffect
    .split('+')
    .map((effect) => effect.trim())
    .includes('Blackout'),
)
</script>
