<template>
  <img
    :src="corrupted ? glitchLogoUrl : normalLogoUrl"
    alt="LingChatLogo"
    class="absolute
      top-0
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
    v-bind="$attrs"
  />
</template>

<script setup lang="ts">
import normalLogoUrl from '../../../../assets/images/LingChatLogo.png'
import glitchLogoUrl from '../../../../assets/images/LingChatLogoGlitch.png'

defineOptions({ inheritAttrs: false })

// 剧本主菜单特效（blood/ghost）生效时切换为崩坏 logo
defineProps({
  corrupted: {
    type: Boolean,
    default: false,
  },
})
</script>
