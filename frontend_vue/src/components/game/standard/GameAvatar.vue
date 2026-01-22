<template>
  <div :class="containerClasses" class="absolute w-full h-full" @animationend="handleAnimationEnd">
    <!-- 核心修改：双层图片结构 -->
    <!-- 1. 底层：显示当前稳定的老图片 -->
    <div
      class="avatar-layer base-layer"
      :style="{ backgroundImage: `url(${currentAvatarUrl})` }"
    ></div>

    <!-- 2. 顶层：负责淡入新图片 -->
    <div
      class="avatar-layer overlay-layer"
      :class="{ 'is-fading-in': isFadingIn }"
      :style="{ backgroundImage: `url(${nextAvatarUrl})` }"
      @transitionend="onTransitionEnd"
    ></div>
    <!-- 修改结束 -->

    <div :class="bubbleClasses" :style="bubbleStyles" class="bubble"></div>

    <!-- 主音频播放器 -->
    <audio ref="avatarAudio" @ended="onAudioEnded"></audio>
    <!-- 气泡效果音播放器 -->
    <audio ref="bubbleAudio"></audio>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue'
import { API_CONFIG } from '@/controllers/core/config'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { EMOTION_CONFIG, EMOTION_CONFIG_EMO } from '@/controllers/emotion/config'
import './avatar-animation.css'

const gameStore = useGameStore()
const uiStore = useUIStore()
const emit = defineEmits(['audio-ended'])

const avatarAudio = ref<HTMLAudioElement | null>(null)
const bubbleAudio = ref<HTMLAudioElement | null>(null)
const activeAnimationClass = ref('normalx')

const currentAvatarUrl = ref('') // 底层图片（老图）
const nextAvatarUrl = ref('') // 顶层图片（新图）
const isFadingIn = ref(false) // 控制顶层图片的淡入状态

const isBubbleVisible = ref(false)
const currentBubbleImageUrl = ref('')
const currentBubbleClass = ref('')

// 计算目标 URL (不直接用于显示，而是作为加载源)
const targetAvatarUrl = computed(() => {
  const character = gameStore.character
  const clothes_name = gameStore.avatar.clothes_name ?? 'default'
  const emotion = gameStore.avatar.emotion

  const emotionConfig = EMOTION_CONFIG[emotion] || EMOTION_CONFIG['正常']

  if (emotion === 'AI思考') return 'none'

  const avatarUrl = `${emotionConfig?.avatar}/${clothes_name}`
  if (!gameStore.script.isRunning) return `${emotionConfig?.avatar ? avatarUrl : ''}`

  return `/api/v1/chat/character/get_script_avatar/${character}/${clothes_name}/${EMOTION_CONFIG_EMO[emotion]}`
})

const containerClasses = computed(() => ({
  [activeAnimationClass.value]: true,
  'avatar-visible': gameStore.avatar.show,
  'avatar-hidden': !gameStore.avatar.show,
}))

// 注意：这里不再计算 avatarStyles 给 div 用，而是直接在 template 里写 style
const bubbleClasses = computed(() => ({
  show: isBubbleVisible.value,
  [currentBubbleClass.value]: isBubbleVisible.value && currentBubbleClass.value,
}))

const bubbleStyles = computed(() => ({
  left: `${gameStore.avatar.bubble_left}%`,
  top: `${gameStore.avatar.bubble_top}%`,
  backgroundImage: `url(${currentBubbleImageUrl.value})`,
}))

const updateAvatarImage = async (newUrl: string) => {
  if (!newUrl || newUrl === 'none') return

  const timestamp = Date.now()
  const finalUrl = `${newUrl}?t=${timestamp}` // 添加时间戳防止缓存

  // TODO: API更改后，这里可以做优化
  // 如果目标图片和当前底层显示的图片一样，且没有正在进行的转场，直接忽略
  //if (newUrl === currentAvatarUrl.value && !isFadingIn.value) return

  // 如果目标图片和正在淡入的图片一样，也忽略
  // if (newUrl === nextAvatarUrl.value && isFadingIn.value) return

  // 1. 预加载图片
  const img = new Image()
  img.src = finalUrl

  try {
    await img.decode()

    // 图片加载完成！ TODO: 之后的优化中，可以在这里才触发情绪切换事件，保证情绪切换和表情切换同步

    // 如果当前正在进行动画（快速切换的情况）：
    // 立即强制完成上一次动画：把上一张图定死在底层，重置状态
    if (isFadingIn.value) {
      currentAvatarUrl.value = nextAvatarUrl.value
      isFadingIn.value = false
      // 等待 DOM 更新一帧，确保 class 被移除，防止 CSS transition 冲突
      await nextTick()
    }

    // 2. 准备下一帧
    nextAvatarUrl.value = finalUrl

    // 3. 开始淡入 (这里加一个小延时或者 requestAnimationFrame 确保浏览器渲染了 dom 的初始状态 opacity:0)
    requestAnimationFrame(() => {
      isFadingIn.value = true
    })
  } catch (err) {
    console.error(`加载头像失败: ${newUrl}`, err)
    currentAvatarUrl.value = '/api/v1/chat/character/get_avatar/正常.png'
  }
}

// 监听 CSS transition 结束事件
const onTransitionEnd = () => {
  if (isFadingIn.value) {
    // 动画完成：新图已经完全盖住了老图
    // 1. 把新图“晋升”为老图（底层）
    currentAvatarUrl.value = nextAvatarUrl.value

    // 2. 瞬间把顶层图隐藏（重置状态）
    // 因为此时底层和顶层图一样，用户肉眼看不出变化，实现了无缝衔接
    isFadingIn.value = false
  }
}

// ----------------------------

watch(
  targetAvatarUrl,
  (newUrl) => {
    updateAvatarImage(newUrl as string)
  },
  { immediate: true },
)

watch(
  () => gameStore.avatar.character_id,
  () => updateAvatarImage(targetAvatarUrl.value as string),
)

watch(
  () => gameStore.avatar.clothes_name,
  () => updateAvatarImage(targetAvatarUrl.value as string),
)

watch(
  () => gameStore.avatar.emotion,
  (newEmotion) => {
    const config = EMOTION_CONFIG[newEmotion]
    if (!config) return

    // 动画类名控制
    if (config.animation && config.animation !== 'none') {
      activeAnimationClass.value = config.animation
    }

    // 气泡逻辑
    if (config.bubbleImage && config.bubbleImage !== 'none') {
      const version = Date.now()
      currentBubbleImageUrl.value = `${config.bubbleImage}?t=${version}#t=0.1`
      currentBubbleClass.value = config.bubbleClass
      isBubbleVisible.value = false
      nextTick(() => {
        isBubbleVisible.value = true
      })
      setTimeout(() => {
        isBubbleVisible.value = false
      }, 2000)
    }

    // 音频逻辑
    if (config.audio && config.audio !== 'none' && bubbleAudio.value) {
      bubbleAudio.value.src = config.audio
      bubbleAudio.value.load()
      bubbleAudio.value.play()
    }
  },
  { immediate: true },
)

// 音频监听保持不变
watch(
  () => uiStore.currentAvatarAudio,
  (newAudio) => {
    if (avatarAudio.value && newAudio && newAudio !== 'None') {
      avatarAudio.value.src = `${API_CONFIG.VOICE.BASE}/${newAudio}`
      avatarAudio.value.load()
      avatarAudio.value.play()
    }
  },
)

watch(
  () => uiStore.characterVolume,
  (v) => {
    if (avatarAudio.value) avatarAudio.value.volume = v / 100
  },
)
watch(
  () => uiStore.bubbleVolume,
  (v) => {
    if (bubbleAudio.value) bubbleAudio.value.volume = v / 100
  },
)

const onAudioEnded = () => emit('audio-ended')
const handleAnimationEnd = () => {
  if (activeAnimationClass.value !== 'normal') {
    activeAnimationClass.value = 'normal'
  }
}
const setEmotion = (emotion: string) => {
  gameStore.avatar.emotion = emotion
}

defineExpose({ setEmotion })
</script>

<style>
/* 通用层样式 */
.avatar-layer {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 102%;
  background-size: contain;
  background-position: center center;
  background-repeat: no-repeat;
  transform-origin: center 0%;

  /* 确保不因为硬件加速导致闪烁 */
  backface-visibility: hidden;
  will-change: opacity, background-image;
}

.base-layer {
  z-index: 1;
}

.overlay-layer {
  z-index: 2;
  opacity: 0;
  transition: opacity 0.3s ease-in-out;
}

.overlay-layer.is-fading-in {
  opacity: 1;
}

.avatar-layer {
  top: v-bind("gameStore.avatar.offset_y + 'px'");
  transform: scale(v-bind('gameStore.avatar.scale'));
}
</style>
