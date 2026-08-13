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
        ref="dialogRef"
        tabindex="-1"
        class="fixed inset-0 z-[9999] flex items-end justify-center outline-none"
        @click.self="onBackdropClick"
        @keydown.esc="onEsc"
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
          :aria-label="$t('ui.imageSourcePicker.title')"
          class="relative w-full max-w-md mx-3 mb-6 rounded-2xl bg-neutral-900/95 border border-white/10 shadow-2xl overflow-hidden"
        >
          <div class="px-5 pt-5 pb-2 text-white/90 text-sm font-medium">
            {{ $t('ui.imageSourcePicker.title') }}
          </div>

          <button
            type="button"
            class="w-full flex items-center gap-3 px-5 py-4 text-left text-white hover:bg-white/10 active:bg-white/15 transition-colors"
            @click="onCamera"
          >
            <Camera :size="20" class="text-cyan-400 shrink-0" />
            <div class="flex-1">
              <div class="text-base">{{ $t('ui.imageSourcePicker.camera') }}</div>
              <div class="text-xs text-white/50">{{ $t('ui.imageSourcePicker.cameraDesc') }}</div>
            </div>
          </button>

          <button
            type="button"
            class="w-full flex items-center gap-3 px-5 py-4 text-left text-white hover:bg-white/10 active:bg-white/15 transition-colors border-t border-white/5"
            @click="onGallery"
          >
            <Image :size="20" class="text-cyan-400 shrink-0" />
            <div class="flex-1">
              <div class="text-base">{{ $t('ui.imageSourcePicker.gallery') }}</div>
              <div class="text-xs text-white/50">{{ $t('ui.imageSourcePicker.galleryDesc') }}</div>
            </div>
          </button>

          <button
            type="button"
            class="w-full px-5 py-3 text-white/70 text-sm hover:bg-white/10 active:bg-white/15 transition-colors border-t border-white/10"
            @click="onCancel"
          >
            {{ $t('ui.imageSourcePicker.cancel') }}
          </button>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, onUnmounted, watch, nextTick } from 'vue'
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

const dialogRef = ref<HTMLDivElement | null>(null)

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

function onEsc() {
  cancel()
}

// 用户切到后台(扣 home / 拉下通知中心 / 弹出权限框等)时自动 cancel,
// 避免 useScreenshot.isCapturing 永远卡 true。
// 仅在 picker 会话真正打开时取消,桌面端 isOpen 恒为 false,不会误触发 cancel_screenshot。
function onVisibilityChange() {
  if (
    typeof document !== 'undefined' &&
    document.visibilityState === 'hidden' &&
    isOpen.value
  ) {
    cancel()
  }
}

// sheet 打开时自动 focus 容器,以接收 keydown.esc。
watch(
  isOpen,
  (open) => {
    if (open) {
      nextTick(() => dialogRef.value?.focus())
    }
  },
)

onMounted(() => {
  init()
  document.addEventListener('visibilitychange', onVisibilityChange)
})

// 路由切换 / 父组件卸载时兜底 cancel,防止 isCapturing 卡死。
// 仅当 picker 会话真正打开时才取消,避免桌面端切路由误触发 cancel_screenshot。
onBeforeUnmount(() => {
  if (isOpen.value) cancel()
})

onUnmounted(() => {
  document.removeEventListener('visibilitychange', onVisibilityChange)
  destroy()
})
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
