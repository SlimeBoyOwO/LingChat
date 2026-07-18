<!--
  ImageSourcePicker

  Android 端的"截屏"功能入口 —— 弹出一个底部 sheet 让用户选拍照或从相册选图。
  桌面端不显示(后端不会 emit `screenshot:request-source`)。

  通过 <Teleport to="body"> 挂到 body,避免被外层 CSS zoom 干扰布局。
-->
<template>
  <Teleport to="body">
    <Transition name="picker-fade">
      <div
        v-if="isOpen"
        class="fixed inset-0 z-[9999] flex items-end justify-center"
        @click.self="onBackdropClick"
      >
        <!-- 背景遮罩 -->
        <div
          class="absolute inset-0 bg-black/60 backdrop-blur-sm"
          aria-hidden="true"
        ></div>

        <!-- 底部 sheet -->
        <div
          role="dialog"
          aria-modal="true"
          aria-label="选择图片来源"
          class="relative w-full max-w-md mx-3 mb-6 rounded-2xl bg-neutral-900/95 border border-white/10 shadow-2xl overflow-hidden"
        >
          <div class="px-5 pt-5 pb-2 text-white/90 text-sm font-medium">
            选择图片来源
          </div>

          <button
            type="button"
            class="w-full flex items-center gap-3 px-5 py-4 text-left text-white hover:bg-white/10 active:bg-white/15 transition-colors"
            @click="onCamera"
          >
            <Camera :size="20" class="text-cyan-400 shrink-0" />
            <div class="flex-1">
              <div class="text-base">拍照</div>
              <div class="text-xs text-white/50">使用相机拍摄一张新图片</div>
            </div>
          </button>

          <button
            type="button"
            class="w-full flex items-center gap-3 px-5 py-4 text-left text-white hover:bg-white/10 active:bg-white/15 transition-colors border-t border-white/5"
            @click="onGallery"
          >
            <Image :size="20" class="text-cyan-400 shrink-0" />
            <div class="flex-1">
              <div class="text-base">从相册选择</div>
              <div class="text-xs text-white/50">从设备相册中选一张图片</div>
            </div>
          </button>

          <button
            type="button"
            class="w-full px-5 py-3 text-white/70 text-sm hover:bg-white/10 active:bg-white/15 transition-colors border-t border-white/10"
            @click="onCancel"
          >
            取消
          </button>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { Camera, Image } from 'lucide-vue-next'
import { useImageSourcePicker } from '@/composables/useImageSourcePicker'

const {
  isOpen,
  init,
  destroy,
  pickFromCamera,
  pickFromGallery,
  cancel,
} = useImageSourcePicker()

async function onCamera() {
  await pickFromCamera()
}

async function onGallery() {
  await pickFromGallery()
}

async function onCancel() {
  await cancel()
}

function onBackdropClick() {
  cancel()
}

onMounted(() => init())
onUnmounted(() => destroy())
</script>

<style scoped>
.picker-fade-enter-active,
.picker-fade-leave-active {
  transition: opacity 0.18s ease-out;
}
.picker-fade-enter-from,
.picker-fade-leave-to {
  opacity: 0;
}
</style>
