<template>
  <!-- 触摸区域 -->
  <TouchAreas v-if="gameStore.command === 'touch'" :body-parts="role.bodyPart" />

  <Transition name="character-fade">
    <div
      class="absolute w-full h-full pointer-events-none origin-[center_0%] role-container-transition"
      :style="containerStyle"
      @animationend="handleAnimationEnd"
    >
      <!-- 使用单独提取出来的图片淡入淡出组件 -->
      <ImageCrossFade
        ref="imageFadeRef"
        class="absolute w-full h-[102%]"
        :class="containerClasses"
        :src="targetAvatarUrl"
        position="center bottom"
        object-fit="contain"
      />

      <!-- 气泡 -->
      <div :class="bubbleClasses" :style="bubbleStyles" class="bubble"></div>

      <!-- 情绪音效 -->
      <audio ref="bubbleAudio"></audio>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, toRefs } from 'vue'
import { useGameStore } from '@/stores/modules/game'
import { EMOTION_CONFIG, EMOTION_CONFIG_EMO } from '@/controllers/emotion/config'
import type { GameRole } from '@/stores/modules/game/state'
import TouchAreas from './TouchAreas.vue'
import ImageCrossFade from '@/components/ui/ImageAcrossFade.vue'
import './avatar-animation.css'

const props = defineProps<{
  role: GameRole
}>()

const gameStore = useGameStore()
const { role } = toRefs(props)

const bubbleAudio = ref<HTMLAudioElement | null>(null)
const imageFadeRef = ref<InstanceType<typeof ImageCrossFade> | null>(null)

const activeAnimationClass = ref('normal')
const isBubbleVisible = ref(false)
const currentBubbleImageUrl = ref('')
const currentBubbleClass = ref('')

let bubbleTimeoutId: number | null = null
let latestEmotionId = 0

// --- 样式计算 ---
const layoutPosition = computed(() => {
  const allIds = gameStore.presentRoleIds
  const myIndex = allIds.indexOf(role.value.roleId)
  const totalCount = allIds.length
  if (myIndex === -1) return 50
  return ((myIndex + 1) / (totalCount + 1)) * 100
})

const containerStyle = computed(() => {
  const autoLeft = layoutPosition.value
  const manualOffset = role.value.offsetX || 0

  return {
    left: `calc(${autoLeft}% + ${manualOffset}px)`,
    top: `${role.value.offsetY}px`,
    transform: `translateX(-50%) scale(${role.value.scale})`,
    opacity: `${role.value.show ? 1 : 0}`,
    zIndex: 1,
  }
})

const containerClasses = computed(() => ({
  [activeAnimationClass.value]: true,
}))

const bubbleClasses = computed(() => ({
  show: isBubbleVisible.value,
  [currentBubbleClass.value]: isBubbleVisible.value && currentBubbleClass.value,
}))

const bubbleStyles = computed(() => ({
  left: `${+role.value.bubbleLeft + 5}%`,
  top: `${+role.value.bubbleTop - 5}%`,
  backgroundImage: `url(${currentBubbleImageUrl.value})`,
}))

const targetAvatarUrl = computed(() => {
  const r = role.value
  const clothesName = r.clothesName === '默认' || !r.clothesName ? 'default' : r.clothesName
  const emotion = r.emotion

  const mappedEmotion = EMOTION_CONFIG_EMO[emotion] || '正常'
  if (emotion === 'AI思考') return 'none'

  return `/api/v1/chat/character/get_avatar/${r.roleId}/${mappedEmotion}/${clothesName}`
})

// 监听表情，配合子组件的加载状态播放特效
watch(
  () => role.value.emotion,
  async (newEmotion) => {
    const currentId = ++latestEmotionId

    // 1. 等待 Vue computed(targetAvatarUrl) 更新并传递给子组件
    await nextTick()

    // 2. 等待子组件的图片加载 Promise 结束
    if (imageFadeRef.value) {
      await imageFadeRef.value.waitForLoad()
    }

    const config = EMOTION_CONFIG[newEmotion]
    if (!config) return

    if (config.animation && config.animation !== 'none') {
      activeAnimationClass.value = config.animation
    }

    if (config.bubbleImage && config.bubbleImage !== 'none') {
      const version = Date.now()
      currentBubbleImageUrl.value = `${config.bubbleImage}?t=${version}#t=0.1`
      currentBubbleClass.value = config.bubbleClass
      isBubbleVisible.value = false
      nextTick(() => {
        isBubbleVisible.value = true

        if (bubbleTimeoutId !== null) {
          window.clearTimeout(bubbleTimeoutId)
        }
        bubbleTimeoutId = window.setTimeout(() => {
          isBubbleVisible.value = false
          bubbleTimeoutId = null
        }, 2000)
      })
    }

    if (config.audio && config.audio !== 'none' && bubbleAudio.value) {
      bubbleAudio.value.src = config.audio
      bubbleAudio.value.load()
      bubbleAudio.value.play()
    }
  },
  { immediate: true },
)

const handleAnimationEnd = () => {
  if (activeAnimationClass.value !== 'normal') {
    activeAnimationClass.value = 'normal'
  }
}
</script>

<style scoped>
.role-container-transition {
  transition:
    left 0.5s cubic-bezier(0.25, 0.8, 0.5, 1),
    top 0.3s ease,
    opacity 0.3s ease-in-out;
}

/* --- 角色进场/退场动画 (Vue Transition 组件必需的样式) --- */
.character-fade-enter-active,
.character-fade-leave-active {
  transition:
    opacity 0.5s ease-in-out,
    transform 0.5s ease-out;
}

.character-fade-enter-from,
.character-fade-leave-to {
  opacity: 0;
}

:deep(.touch-area) {
  pointer-events: auto;
}
</style>
