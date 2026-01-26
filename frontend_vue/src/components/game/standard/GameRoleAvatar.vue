<template>
  <div
    class="role-avatar-container absolute"
    :style="containerStyle"
    @animationend="handleAnimationEnd"
  >
    <!-- 双层图片结构 (保持原有平滑切换逻辑) -->
    <div class="w-full h-full absolute" :class="containerClasses">
      <div
        class="avatar-layer base-layer"
        :style="{ backgroundImage: `url(${currentAvatarUrl})` }"
      ></div>

      <div
        class="avatar-layer overlay-layer"
        :class="{ 'is-fading-in': isFadingIn }"
        :style="{ backgroundImage: `url(${nextAvatarUrl})` }"
        @transitionend="onTransitionEnd"
      ></div>
    </div>

    <!-- 触摸区域 -->
    <TouchAreas
      v-for="(part, key) in role.bodyPart"
      :key="key"
      :game-store="gameStore"
      :part="part"
      :part-key="String(key)"
      :role-id="role.roleId"
    />

    <!-- 气泡 -->
    <div :class="bubbleClasses" :style="bubbleStyles" class="bubble"></div>

    <!-- 情绪音效播放器 (每个角色独立) -->
    <audio ref="bubbleAudio"></audio>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, toRefs } from 'vue'
import { useGameStore } from '@/stores/modules/game'
import { EMOTION_CONFIG, EMOTION_CONFIG_EMO } from '@/controllers/emotion/config'
import type { GameRole } from '@/stores/modules/game/state' // 假设路径
import TouchAreas from './TouchAreas.vue'
import './avatar-animation.css'

const props = defineProps<{
  role: GameRole
}>()

const gameStore = useGameStore()
const { role } = toRefs(props) // 保持响应式

// --- 本地状态 ---
const bubbleAudio = ref<HTMLAudioElement | null>(null)
const activeAnimationClass = ref('normal')

const currentAvatarUrl = ref('')
const nextAvatarUrl = ref('')
const isFadingIn = ref(false)

const isBubbleVisible = ref(false)
const currentBubbleImageUrl = ref('')
const currentBubbleClass = ref('')

// --- 样式计算 ---

const layoutPosition = computed(() => {
  const allIds = gameStore.presentRoleIds
  const myIndex = allIds.indexOf(role.value.roleId)
  const totalCount = allIds.length

  // 如果找不到自己（异常情况），默认居中
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
    display: role.value.show ? 'block' : 'none',
    zIndex: 1,
  }
})

const containerClasses = computed(() => ({
  [activeAnimationClass.value]: true,
  'avatar-visible': role.value.show,
  'avatar-hidden': !role.value.show,
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

// --- 图片 URL 计算 ---
const targetAvatarUrl = computed(() => {
  const r = role.value
  const clothesName = r.clothesName ?? 'default'
  const emotion = r.emotion

  const mappedEmotion = EMOTION_CONFIG_EMO[emotion] || '正常'
  if (emotion === 'AI思考') return 'none'

  return `/api/v1/chat/character/get_avatar/${r.roleId}/${mappedEmotion}/${clothesName}`
})

// --- 核心逻辑：图片切换 (复用原有逻辑) ---
const updateAvatarImage = async (newUrl: string) => {
  if (!newUrl || newUrl === 'none') return

  const finalUrl = `${newUrl}`

  const img = new Image()
  img.src = finalUrl

  try {
    await img.decode()

    if (isFadingIn.value) {
      currentAvatarUrl.value = nextAvatarUrl.value
      isFadingIn.value = false
      await nextTick()
    }

    nextAvatarUrl.value = finalUrl
    requestAnimationFrame(() => {
      isFadingIn.value = true
    })
  } catch (err) {
    console.error(`角色[${role.value.roleName}]加载头像失败: ${newUrl}`, err)
    // 降级处理逻辑...
  }
}

const onTransitionEnd = () => {
  if (isFadingIn.value) {
    currentAvatarUrl.value = nextAvatarUrl.value
    isFadingIn.value = false
  }
}

// --- Watchers ---

// 1. 监听 URL 变化触发加载
watch(targetAvatarUrl, (newUrl) => updateAvatarImage(newUrl), { immediate: true })

// 2. 监听情绪变化处理 动画/气泡/音效
watch(
  () => role.value.emotion,
  (newEmotion) => {
    const config = EMOTION_CONFIG[newEmotion]
    if (!config) return

    // 动画类名
    if (config.animation && config.animation !== 'none') {
      activeAnimationClass.value = config.animation
    }

    // 气泡
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

    // 情绪音效 (局部)
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
/* 组件内部样式，负责具体的图层叠放 */
.role-avatar-container {
  /* 宽高建议由图片撑开或父级指定，这里假设由内容决定 */
  transform-origin: center 0%;
  width: 100%;
  height: 100%;
  pointer-events: none; /* 让点击穿透，除非点到 TouchArea */
  transition:
    left 0.5s cubic-bezier(0.25, 0.8, 0.5, 1),
    top 0.3s ease;
}

/* 让 TouchArea 恢复点击 */
:deep(.touch-area) {
  pointer-events: auto;
}

.avatar-layer {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 102%;
  background-size: contain;
  background-position: center bottom;
  background-repeat: no-repeat;

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
</style>
