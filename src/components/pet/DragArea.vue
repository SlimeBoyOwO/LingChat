<script setup lang="ts">
import { useFileDrop } from './useFileDrop'

const { isDragging } = useFileDrop()
</script>

<template>
  <div class="drop-zone" :class="{ dragging: isDragging }">
    <slot></slot>
  </div>
</template>

<style scoped>
.drop-zone {
  position: relative;
}
.dragging {
  outline: 2px dashed blue;
}
</style>