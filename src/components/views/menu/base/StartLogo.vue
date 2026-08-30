<template>
  <img
    src="../../../../assets/images/LingChatLogo.png"
    alt="LingChatLogo"
    class="absolute
      drop-shadow-[0_4px_8px_rgba(0,0,0,0.3)]
      transition-transform
      hover:scale-105
      right-0
      z-5
      w-auto
      cursor-pointer
      duration-300
      max-w-[40vw]
      h-[16vw]"
    :style="{ top: 'var(--safe-area-inset-top, 0px)', right: 'var(--safe-area-inset-right, 0px)' }"
    v-bind="$attrs"
  />
</template>

<script setup lang="ts">
defineOptions({ inheritAttrs: false })
</script>
