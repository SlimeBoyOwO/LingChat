<template>
  <div class="relative w-full h-full">
    <!-- 底层图片（当前显示的图片） -->
    <div
      class="absolute inset-0 w-full h-full bg-no-repeat z-10 backface-hidden will-change-[opacity,background-image]"
      :style="{
        backgroundImage: `url(${currentImageUrl})`,
        backgroundSize: objectFit,
        backgroundPosition: position,
      }"
    ></div>

    <!-- 顶层图片（准备淡入的新图片） -->
    <div
      class="absolute inset-0 w-full h-full bg-no-repeat z-20 backface-hidden will-change-[opacity,background-image] transition-opacity ease-in-out"
      :class="isFadingIn ? 'opacity-100' : 'opacity-0'"
      :style="{
        backgroundImage: `url(${nextImageUrl})`,
        backgroundSize: objectFit,
        backgroundPosition: position,
        transitionDuration: `${duration}ms`,
      }"
      @transitionend="onTransitionEnd"
    ></div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, nextTick } from 'vue'

const props = withDefaults(
  defineProps<{
    src: string
    duration?: number // 过渡动画时间 (ms)
    objectFit?: string // 对应 background-size
    position?: string // 对应 background-position
  }>(),
  {
    duration: 300,
    objectFit: 'contain',
    position: 'center bottom',
  },
)

const currentImageUrl = ref('')
const nextImageUrl = ref('')
const isFadingIn = ref(false)

// 记录当前正在加载的 Promise，防止并发竞态
let currentImageLoadPromise: Promise<void> | null = null

const updateImage = async (newUrl: string) => {
  if (!newUrl || newUrl === 'none') return

  let resolveLoad!: () => void
  const loadPromise = new Promise<void>((resolve) => {
    resolveLoad = resolve
  })
  currentImageLoadPromise = loadPromise

  const img = new Image()
  img.src = newUrl
  try {
    await img.decode()
  } catch (err) {
    console.error(`加载图片失败: ${newUrl}`, err)
  }

  // 确保只有最后一次触发的加载才会执行 DOM 更新
  if (currentImageLoadPromise === loadPromise) {
    if (isFadingIn.value) {
      currentImageUrl.value = nextImageUrl.value
      isFadingIn.value = false
      await nextTick()
    }
    nextImageUrl.value = newUrl
    requestAnimationFrame(() => {
      isFadingIn.value = true
    })
  }

  resolveLoad()
}

const onTransitionEnd = () => {
  if (isFadingIn.value) {
    currentImageUrl.value = nextImageUrl.value
    isFadingIn.value = false
  }
}

// 暴露等待图片加载完成的方法给父组件
const waitForLoad = () => currentImageLoadPromise || Promise.resolve()

defineExpose({
  waitForLoad,
})

watch(
  () => props.src,
  (newUrl) => {
    updateImage(newUrl)
  },
  { immediate: true },
)
</script>
