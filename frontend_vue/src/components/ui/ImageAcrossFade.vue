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
    <!-- 新增 ref="topDivRef" 用于强制重排 -->
    <div
      ref="topDivRef"
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
    duration?: number
    objectFit?: string
    position?: string
  }>(),
  {
    duration: 300,
    objectFit: 'contain',
    position: 'center bottom',
  },
)

const topDivRef = ref<HTMLElement | null>(null) // 获取顶层 DOM 的引用
const currentImageUrl = ref('')
const nextImageUrl = ref('')
const isFadingIn = ref(false)

let currentImageLoadPromise: Promise<void> | null = null

const updateImage = async (newUrl: string) => {
  if (!newUrl || newUrl === 'none') return

  let resolveLoad!: () => void
  const loadPromise = new Promise<void>((resolve) => {
    resolveLoad = resolve
  })
  currentImageLoadPromise = loadPromise

  // 1. 完善的图片预加载机制
  const img = new Image()
  const imgReadyPromise = new Promise<void>((resolve, reject) => {
    img.onload = () => resolve()
    img.onerror = (err) => reject(err)
  })
  img.src = newUrl

  try {
    // 必须先等网络请求完全结束
    await imgReadyPromise
    // 然后再等 CPU 解码完成 (忽略 decode 本身不支持时的报错)
    await img.decode().catch(() => {})
  } catch (err) {
    console.error(`加载图片失败: ${newUrl}`, err)
  }

  // 确保只有最后一次触发的加载才会执行 DOM 更新
  if (currentImageLoadPromise === loadPromise) {
    // 如果上一个动画还没完就被打断，先整理状态
    if (isFadingIn.value) {
      currentImageUrl.value = nextImageUrl.value || currentImageUrl.value
      isFadingIn.value = false
    }

    // 2. 赋值新的背景图
    nextImageUrl.value = newUrl

    // 3. 关键：等待 Vue 将 URL 更新到真实 DOM (style属性中)
    await nextTick()

    // 4. 关键：强制浏览器重排 (Reflow)
    // 这行代码会让浏览器立即停下来，去读取一遍新图片的样式
    // 此时它处于 opacity: 0 的状态，浏览器会默默在后台把它准备好
    if (topDivRef.value) {
      void topDivRef.value.offsetHeight
    }

    // 5. 在下一帧安全地开启淡入动画
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
