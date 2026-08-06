<template>
  <Teleport to="body">
    <div
      class="modal-mask
        fixed
        inset-0
        z-[9999]
        flex
        items-center
        justify-center
        p-4
        backdrop-blur-md
        bg-black/55"
      @click.self="emit('cancel')"
    >
      <div
        class="flex
          w-[min(760px,92vw)]
          flex-col
          overflow-hidden
          rounded-xl
          border
          border-white/12.5
          bg-[rgba(12,20,30,0.94)]
          shadow-[0_8px_32px_rgba(0,0,0,0.45)]"
      >
        <div class="flex
          items-center
          justify-between
          gap-4
          border-b
          border-white/10
          px-5
          py-3">
          <h4 class="font-semibold
            text-white">{{ t('scriptEditor.imageCrop.title') }}</h4>
          <span class="text-[0.7rem]
            text-white/45">{{ t('scriptEditor.imageCrop.hint') }}</span>
        </div>

        <div class="relative
          max-h-[52vh]
          min-h-[300px]
          overflow-hidden
          bg-black/70">
          <img
            ref="imgEl"
            :src="imgSrc"
            class="max-w-full"
            :alt="t('scriptEditor.imageCrop.previewAlt')"
          />
        </div>

        <div class="flex
          items-center
          justify-end
          gap-2
          px-5
          py-3">
          <button
            class="rounded-lg
              border
              border-white/10
              bg-white/6
              px-4
              py-1.5
              text-[0.8rem]
              text-white/70
              transition-colors
              hover:bg-white/[0.12]
              hover:text-white"
            @click="emit('cancel')"
          >
            {{ t('scriptEditor.imageCrop.cancel') }}
          </button>
          <button
            class="rounded-lg
              border
              border-brand/45
              bg-brand/14
              px-4
              py-1.5
              text-[0.8rem]
              text-brand
              transition-colors
              hover:bg-brand/24"
            @click="confirm"
          >
            {{ t('scriptEditor.imageCrop.confirm') }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import Cropper from 'cropperjs'
import 'cropperjs/dist/cropper.css'
import { convertFileSrc } from '@tauri-apps/api/core'

const { t } = useI18n()

const props = defineProps<{ srcPath: string }>()
const emit = defineEmits<{
  confirm: [dataUrl: string, name: string]
  cancel: []
}>()

const imgEl = ref<HTMLImageElement | null>(null)
let cropper: Cropper | null = null

/** 原图以 asset URL 加载（与编辑器背景同一套资源访问方式） */
const imgSrc = convertFileSrc(props.srcPath)

// 选区比例取当前编辑器窗口宽高比：背景是 cover 铺满窗口，只有比例一致时
// 裁剪框内看到的内容才等于最终显示，不会再有"看不见的二次裁切"
const windowRatio = window.innerWidth / window.innerHeight

const initCropper = () => {
  if (!imgEl.value || cropper) return
  cropper = new Cropper(imgEl.value, {
    viewMode: 1,
    dragMode: 'move',
    aspectRatio: windowRatio,
    // 默认选区铺满整图 = 不裁剪直接使用，用户拖动缩放后才改变范围
    autoCropArea: 1,
    guides: true,
    center: true,
    background: false,
  })
}

onMounted(() => {
  const img = imgEl.value
  if (!img) return
  // 图片可能已在缓存中（load 不再触发），complete 后直接初始化
  if (img.complete) initCropper()
  else img.addEventListener('load', initCropper)
})

onBeforeUnmount(() => {
  cropper?.destroy()
  cropper = null
})

const confirm = () => {
  if (!cropper) return
  // 输出上限 1920 宽：背景只需铺满窗口，太大浪费内存；webp 保持体积小
  const canvas = cropper.getCroppedCanvas({ maxWidth: 1920, maxHeight: 1080 })
  const dataUrl = canvas.toDataURL('image/webp', 0.92)
  // 输出文件名沿用原图名（去扩展名 + _crop），便于在数据目录里识别
  const raw = props.srcPath.split(/[\\/]/).pop() || 'background'
  const base = raw.replace(/\.[^.]+$/, '')
  emit('confirm', dataUrl, `${base}_crop.webp`)
}
</script>

<style scoped>
/* cropperjs 默认白色主题，覆写为编辑器暗色风格 */
:deep(.cropper-modal) {
  opacity: 0.65;
  background-color: #000;
}
:deep(.cropper-view-box) {
  outline-color: rgba(121, 217, 255, 0.65);
}
:deep(.cropper-line) {
  background-color: rgba(121, 217, 255, 0.75);
}
:deep(.cropper-point) {
  background-color: #79d9ff;
}
:deep(.cropper-container) {
  max-width: 100%;
}
</style>
