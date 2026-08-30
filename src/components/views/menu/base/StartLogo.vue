<template>
  <img
    src="../../../../assets/images/LingChatLogo.png"
    alt="LingChatLogo"
    class="absolute right-0 z-5 h-[16vw] w-auto max-w-[40vw] cursor-pointer
      drop-shadow-[0_4px_8px_rgba(0,0,0,0.3)] transition-transform duration-300 hover:scale-105"
    :style="{ top: 'var(--safe-area-inset-top, 0px)', right: 'var(--safe-area-inset-right, 0px)' }"
    v-bind="$attrs"
  />
</template>

<script setup lang="ts">
  defineOptions({ inheritAttrs: false });
</script>
