<template>
  <!-- 1. 添加 Transition 组件，name 指定为 character-fade -->
  <Transition name="character-fade">
    <div
      class="role-avatar-container absolute"
      :style="containerStyle"
      @animationend="handleAnimationEnd"
    >
      <!-- 双层图片结构  -->
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

      <!-- 触摸区域  -->
      <TouchAreas
        v-for="(part, key) in role.bodyPart"
        :key="key"
        :game-store="gameStore"
        :part="part"
        :part-key="String(key)"
        :role-id="role.roleId"
      />

      <!-- 气泡  -->
      <div :class="bubbleClasses" :style="bubbleStyles" class="bubble"></div>

      <!-- 情绪音效  -->
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
import './avatar-animation.css'

const props = defineProps<{
  role: GameRole
}>()

const gameStore = useGameStore()
const { role } = toRefs(props)

const bubbleAudio = ref<HTMLAudioElement | null>(null)
const activeAnimationClass = ref('normal')
const currentAvatarUrl = ref('')
const nextAvatarUrl = ref('')
const isFadingIn = ref(false)
const isBubbleVisible = ref(false)
const currentBubbleImageUrl = ref('')
const currentBubbleClass = ref('')

let bubbleTimeoutId: number | null = null

let currentImageLoadPromise: Promise<void> | null = null
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
  const clothesName = r.clothesName ?? 'default'
  const emotion = r.emotion

  const mappedEmotion = EMOTION_CONFIG_EMO[emotion] || '正常'
  if (emotion === 'AI思考') return 'none'

  return `/api/v1/chat/character/get_avatar/${r.roleId}/${mappedEmotion}/${clothesName}`
})

const updateAvatarImage = async (newUrl: string) => {
  if (!newUrl || newUrl === 'none') return

  let resolveLoad!: () => void
  const loadPromise = new Promise<void>((resolve) => {
    resolveLoad = resolve
  })
  currentImageLoadPromise = loadPromise // 更新当前的加载任务

  const finalUrl = `${newUrl}`
  const img = new Image()
  img.src = finalUrl
  try {
    await img.decode()
  } catch (err) {
    console.error(`角色[${role.value.roleName}]加载头像失败: ${newUrl}`, err)
  }

  // 如果当前加载任务仍然是最新任务，才执行图片替换
  if (currentImageLoadPromise === loadPromise) {
    if (isFadingIn.value) {
      currentAvatarUrl.value = nextAvatarUrl.value
      isFadingIn.value = false
      await nextTick()
    }
    nextAvatarUrl.value = finalUrl
    requestAnimationFrame(() => {
      isFadingIn.value = true
    })
  }

  // 图片已处理完毕（成功或失败），解析 Promise，允许播放气泡等特效
  resolveLoad()
}

const onTransitionEnd = () => {
  if (isFadingIn.value) {
    currentAvatarUrl.value = nextAvatarUrl.value
    isFadingIn.value = false
  }
}

watch(targetAvatarUrl, (newUrl) => updateAvatarImage(newUrl), { immediate: true })

watch(
  () => role.value.emotion,
  async (newEmotion) => {
    const currentId = ++latestEmotionId

    // 等待 Vue 将 computed(targetAvatarUrl) 更新完毕，让 URL 监听器先执行
    await nextTick()

    // 如果此时正在加载新图片，则等待该图片加载完成
    if (currentImageLoadPromise) {
      await currentImageLoadPromise
    }

    // 校验：如果等待图片加载期间，用户又切换了新表情，则中止旧表情的特效播放
    // if (currentId !== latestEmotionId) return

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
          bubbleTimeoutId = null // 重置记录
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
.role-avatar-container {
  transform-origin: center 0%;
  width: 100%;
  height: 100%;
  pointer-events: none;

  transition:
    left 0.5s cubic-bezier(0.25, 0.8, 0.5, 1),
    top 0.3s ease,
    opacity 0.3s ease-in-out;
}

/* --- 角色进场/退场动画 --- */
.character-fade-enter-active,
.character-fade-leave-active {
  transition:
    opacity 0.5s ease-in-out,
    transform 0.5s ease-out;
}

/* 进入前 和 离开后 的状态 (透明) */
.character-fade-enter-from,
.character-fade-leave-to {
  opacity: 0;
}

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
