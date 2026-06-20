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
      <ImageAcrossFade
        ref="imageFadeRef"
        class="absolute w-full h-[102%]"
        :class="containerClasses"
        :src="targetAvatarUrl"
        :duration="300"
        position="center bottom"
        :object-fit="computedObjectFit"
      />

      <!-- 气泡 -->
      <div :class="bubbleClasses" :style="bubbleStyles" class="bubble"></div>

      <!-- 情绪音效 -->
      <audio ref="bubbleAudio"></audio>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, toRefs, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useGameStore } from '@/stores/modules/game'
import { EMOTION_CONFIG, EMOTION_CONFIG_EMO } from '@/controllers/emotion/config'
import type { GameRole } from '@/stores/modules/game/state'
import TouchAreas from './TouchAreas.vue'
import ImageAcrossFade from '@/components/ui/ImageAcrossFade.vue'
import './avatar-animation.css'

const props = defineProps<{
  role: GameRole
}>()

const gameStore = useGameStore()
const { role } = toRefs(props)

const bubbleAudio = ref<HTMLAudioElement | null>(null)
const imageFadeRef = ref<InstanceType<typeof ImageAcrossFade> | null>(null)

const activeAnimationClass = ref('normal')
const isBubbleVisible = ref(false)
const currentBubbleImageUrl = ref('')
const currentBubbleClass = ref('')

let bubbleTimeoutId: number | null = null
let latestEmotionId = 0

// --- 移动端适配：追踪视口尺寸，窄屏时切换 object-fit ---
const viewportWidth = ref(window.innerWidth)
const viewportHeight = ref(window.innerHeight)

function onResize() {
  viewportWidth.value = window.innerWidth
  viewportHeight.value = window.innerHeight
}

onMounted(() => window.addEventListener('resize', onResize))
onUnmounted(() => window.removeEventListener('resize', onResize))

// 窄屏适配：宽高比 < 1.0 时，高度从 100% 线性过渡到 80%（ratio=0.5 时到达 80%）
const computedObjectFit = computed(() => {
  const ratio = viewportWidth.value / viewportHeight.value
  if (ratio >= 1.0) return 'contain'
  const percent = Math.max(98, 100 - (1.0 - ratio) * 40)
  return `auto ${Math.round(percent)}%`
})

// --- 样式计算 ---
const layoutPosition = computed(() => {
  const allIds = gameStore.presentRoleIds
  const myIndex = allIds.indexOf(role.value.roleId)
  const totalCount = allIds.length
  if (myIndex === -1) return 50
  return ((myIndex + 1) / (totalCount + 1)) * 100
})

const lightingFilter = computed(() => {
  const c = gameStore.currentScene?.lighting?.character
  if (!c) return undefined
  const parts: string[] = []
  if (c.brightness !== 1.0) parts.push(`brightness(${c.brightness})`)
  if (c.contrast !== 1.0) parts.push(`contrast(${c.contrast})`)
  if (c.saturation !== 1.0) parts.push(`saturate(${c.saturation})`)
  if (c.glow_radius > 0) parts.push(`drop-shadow(0 0 ${c.glow_radius}px ${c.glow_color})`)
  if (c.sepia > 0) parts.push(`sepia(${c.sepia})`)
  return parts.length > 0 ? parts.join(' ') : undefined
})

const containerStyle = computed(() => {
  const autoLeft = layoutPosition.value
  const manualOffset = role.value.offsetX || 0

  const style: Record<string, string> = {
    left: `calc(${autoLeft}% + ${manualOffset}px)`,
    top: `${role.value.offsetY}px`,
    transform: `translateX(-50%) scale(${role.value.scale})`,
    opacity: `${role.value.show ? 1 : 0}`,
    zIndex: '1',
  }
  const filter = lightingFilter.value
  if (filter) {
    style.filter = filter
  }
  return style
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

const targetAvatarUrl = ref('')

let resolveAvatarId = 0

async function resolveAvatar() {
  const r = role.value
  const clothesName = r.clothesName === '默认' || !r.clothesName ? 'default' : r.clothesName
  const emotion = r.emotion
  const mappedEmotion = EMOTION_CONFIG_EMO[emotion] || '正常'

  if (emotion === 'AI思考') {
    targetAvatarUrl.value = 'none'
    return
  }

  const currentId = ++resolveAvatarId
  try {
    const path = await invoke<string>('get_avatar_file', {
      characterFolder: r.character_folder,
      emotion: mappedEmotion,
      clothesName,
    })
    if (currentId === resolveAvatarId) {
      targetAvatarUrl.value = convertFileSrc(path)
    }
  } catch {
    if (currentId === resolveAvatarId) {
      targetAvatarUrl.value = ''
    }
  }
}

watch(
  () => [
    role.value.roleId,
    role.value.emotion,
    role.value.clothesName,
    role.value.character_folder,
  ],
  () => resolveAvatar(),
  { immediate: true },
)

// 监听表情，配合子组件的加载状态播放特效
watch(
  () => role.value.emotion,
  async (newEmotion) => {
    const currentId = ++latestEmotionId

    // 1. 等待异步头像路径解析完成
    await resolveAvatar()

    // 2. 等待 Vue 更新 DOM 并传递给子组件
    await nextTick()

    // 3. 等待子组件的图片加载 Promise 结束
    if (imageFadeRef.value) {
      await imageFadeRef.value.waitForLoad()
    }

    // 检查是否仍然是最新的表情更新
    if (currentId !== latestEmotionId) return

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
