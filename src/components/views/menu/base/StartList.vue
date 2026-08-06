<template>
  <nav
    class="flex
      flex-col
      items-stretch"
    v-bind="$attrs"
  >
    <slot />
  </nav>
</template>

<script setup lang="ts">
defineOptions({ inheritAttrs: false })
</script>
