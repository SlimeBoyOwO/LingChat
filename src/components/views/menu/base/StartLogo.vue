<template>
  <img
    src="../../../../assets/images/LingChatLogo.webp"
    alt="LingChatLogo"
    draggable="false"
    class="absolute right-0 z-5 h-[16vw] w-auto max-w-[40vw] cursor-pointer drop-shadow-[0_4px_8px_rgba(0,0,0,0.3)] transition-transform duration-300 hover:scale-105 max-[767px]:h-[24vw] max-[767px]:max-w-[55vw]"
    :style="{ top: 'var(--safe-area-inset-top, 0px)', right: 'var(--safe-area-inset-right, 0px)' }"
    @contextmenu.prevent
    v-bind="$attrs"
  />
</template>

<script setup lang="ts">
defineOptions({ inheritAttrs: false });
</script>
