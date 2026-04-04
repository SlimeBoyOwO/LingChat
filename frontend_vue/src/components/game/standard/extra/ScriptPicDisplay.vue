<template>
  <div
    v-if="showPic"
    class="fixed z-1000 pointer-events-none w-full h-full flex justify-center items-center"
    style="transform: translate(0%, -10%)"
  >
    <ImageAcrossFade
      ref="imageFadeRef"
      class="h-[40vh] w-auto max-w-[80vw] max-h-[60vh] object-contain"
      :style="{ transform: `scale(${uiStore.currentPresentPicScale || 1})` }"
      :src="uiStore.currentPresentPic"
      position="center center"
      object-fit="contain"
      :duration="500"
    >
    </ImageAcrossFade>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'
import ImageAcrossFade from '@/components/ui/ImageAcrossFade.vue'

const uiStore = useUIStore()
const showPic = ref(false)

// 监听 currentPresentPic 变化
watch(
  () => uiStore.currentPresentPic,
  (newVal) => {
    if (newVal && newVal !== '') {
      showPic.value = true
    } else {
      // 无图片时，淡出
      showPic.value = false
    }
  },
  { immediate: true },
)
</script>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.5s ease-in-out;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
