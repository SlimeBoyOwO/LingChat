<template>
  <div>
    <slot></slot>
  </div>
</template>

<script setup lang="ts"></script>

<style scoped>
div {
  display: grid;
  grid-template-columns: 1fr; /* 从多列网格改为单列 */
  gap: 20px;
}
</style>
