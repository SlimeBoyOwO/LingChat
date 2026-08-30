<template>
  <div
    class="relative flex items-center justify-center w-full h-full group"
    @click="handleAvatarClick"
  >
    <!-- 缩放与尺寸控制层 (无位移) -->
    <div class="relative w-full h-full">
      <!-- 1. 右上角信息铭牌 -->
      <div
        class="absolute top-1 -right-4 z-50 flex flex-col items-start pointer-events-none opacity-0 translate-x-4 group-hover:opacity-100 group-hover:translate-x-0 transition-all duration-400 ease-out"
      >
        <div
          class="bg-cyan-500 text-white text-[10px] font-black px-2 py-0.5 rounded-tl-md rounded-br-md italic shadow-sm tracking-wider"
        >
          {{ role.roleName }}
        </div>
        <div
          class="text-cyan-700 dark:text-cyan-300 text-xs font-bold tracking-widest pl-1 drop-shadow-sm uppercase"
        >
          {{ role.roleSubTitle }}
        </div>
      </div>

      <!-- 3. 常驻特效：现代科技感流光圆环 -->
      <div
        class="absolute inset-3 rounded-full border-[1.5px] border-cyan-400/20 animate-pulse-slow pointer-events-none"
      ></div>
      <!-- 流光扫边特效环 -->
      <div
        class="absolute -inset-1 rounded-full pointer-events-none sweep-glow-ring drop-shadow-[0_0_6px_rgba(34,211,238,0.4)]"
      ></div>

      <!-- 5. 核心头像框 -->
      <!--
        data-tauri-drag-region="false" 是刻意的：Tauri 注入的 drag.js 对「裸属性」要求
        事件目标就是标注元素本身（el === composedPath[0]），而下面的头像图片容器铺满整个框，
        事件目标永远是子元素，官方路径其实从未触发过；"false" 让 drag.js 显式跳过，避免它与
        下面的 startWindowDrag 形成双路径。CSS 选择器 [data-tauri-drag-region] 匹配任意值，
        Windows 的 -webkit-app-region: drag 保持原样。
      -->
      <div
        class="relative w-full h-full rounded-full bg-white/10 dark:bg-black/10 backdrop-blur-md border-2 border-white/60 dark:border-white/20 shadow-[0_8px_32px_rgba(0,176,255,0.15)] overflow-hidden flex items-center justify-center transition-colors duration-300 z-10"
        data-tauri-drag-region="false"
        @mousedown="startWindowDrag"
        @dragstart.prevent
      >
        <!-- 下降效果的粒子系统 -->
        <BAParticles
          v-if="uiStore.currentBackgroundEffect === 'BA'"
          class="absolute inset-0 w-full h-full z-0 pointer-events-none"
          :particle-count="60"
          :speed="0.2"
        />

        <StarField
          v-if="uiStore.currentBackgroundEffect === 'StarField'"
          class="absolute inset-0 w-full h-full z-0 pointer-events-none"
        />

        <!-- 头像图片容器 -->
        <div
          :class="['w-full h-full z-10 rounded-full overflow-hidden', containerClasses]"
          @animationend="handleAnimationEnd"
        >
          <div class="w-full h-full origin-top" :style="avatarStyles">
            <div
              v-if="live2dFailed && !targetAvatarUrl"
              class="flex h-full w-full items-center justify-center text-xs text-white/60"
            >
              {{ $t('game.avatar.live2dUnavailable') }}
            </div>
            <ImageCrossFade
              v-show="!live2dActive"
              ref="imageFadeRef"
              class="w-full h-full object-cover animate-breathing"
              :src="targetAvatarUrl"
              :style="imageStyles"
              position="center 0%"
              object-fit="cover"
            />
          </div>
        </div>

        <audio ref="bubbleAudio"></audio>
      </div>

      <!-- 6. 气泡表情 -->
      <div
        :class="[
          'absolute w-full h-full top-[-2%] left-[-2%] z-73 bg-contain bg-no-repeat pointer-events-none transition-all duration-300 origin-bottom-left',
          bubbleClasses,
        ]"
        :style="bubbleStyles"
      ></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, toRefs } from 'vue'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import BAParticles from './BAParticles.vue'
import ImageCrossFade from '@/components/ui/ImageAcrossFade.vue'
import StarField from '../game/standard/particles/StarField.vue'
import type { GameRole } from '@/stores/modules/game/state'
import { useGameStore } from '@/stores/modules/game'
import { EMOTION_CONFIG, EMOTION_CONFIG_EMO } from '@/controllers/emotion/config'
import { useUIStore } from '@/stores/modules/ui/ui'
import './avatar-animation.css'

const props = defineProps<{ role: GameRole; live2dActive?: boolean; live2dFailed?: boolean }>()
const { role } = toRefs(props)

const emit = defineEmits(['avatar-click'])
const bubbleAudio = ref<HTMLAudioElement | null>(null)
const imageFadeRef = ref<InstanceType<typeof ImageCrossFade> | null>(null)
const uiStore = useUIStore()
const gameStore = useGameStore()

// ─── 窗口拖曳 ────────────────────────────────────────────────
// macOS 的 WKWebView 不支持 -webkit-app-region: drag，桌宠窗口因此完全拖不动。
// 这里手动接管：按下后位移超过阈值才进入原生窗口拖曳，未超过则保持为普通点击
// （头像的 click 仍会派发，"点击头像推进对话"不受影响）。
const DRAG_THRESHOLD_PX = 4

const startWindowDrag = (e: MouseEvent) => {
  if (e.button !== 0) return

  // 抑制文本选中与 <img> 的原生拖曳：原生 image drag 一旦启动，mousemove 就断流，
  // 阈值永远达不到，拖曳会在整个头像区域间歇性失效。preventDefault 不影响后续 click 派发。
  e.preventDefault()

  const startX = e.screenX
  const startY = e.screenY

  const cleanup = () => {
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', cleanup)
  }

  const onMove = (moveEvent: MouseEvent) => {
    if (
      Math.abs(moveEvent.screenX - startX) < DRAG_THRESHOLD_PX &&
      Math.abs(moveEvent.screenY - startY) < DRAG_THRESHOLD_PX
    ) {
      return
    }
    // 交给系统接管后 webview 收不到后续鼠标事件，先摘监听器再启动拖曳
    cleanup()
    void getCurrentWindow().startDragging()
  }

  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', cleanup)
}

const activeAnimationClass = ref('normal')
const isBubbleVisible = ref(false)
const currentBubbleImageUrl = ref('')
const currentBubbleClass = ref('')

let bubbleTimeoutId: number | null = null
let latestEmotionId = 0

const containerClasses = computed(() => ({
  [activeAnimationClass.value]: true,
  'opacity-100': role.value.show,
  'opacity-0': !role.value.show,
}))

const avatarStyles = computed(() => ({
  transform: `scale(${role.value.scaleP}) translate(${role.value.offsetXP}px, ${role.value.offsetYP}px)`,
}))

const imageStyles = computed(() => ({
  top: `-10px`,
}))

const bubbleClasses = computed(() => ({
  'opacity-100': isBubbleVisible.value,
  'opacity-0': !isBubbleVisible.value,
  [currentBubbleClass.value]: isBubbleVisible.value && currentBubbleClass.value,
}))

const bubbleStyles = computed(() => ({
  backgroundImage: `url(${currentBubbleImageUrl.value})`,
}))

const handleAvatarClick = () => emit('avatar-click')

const handleAnimationEnd = () => {
  if (activeAnimationClass.value !== 'normal') {
    activeAnimationClass.value = 'normal'
  }
}

const targetAvatarUrl = ref('')
let resolveAvatarId = 0

async function resolveAvatar() {
  const r = role.value
  const clothesName = r.clothesName === '默认' || !r.clothesName ? 'default' : r.clothesName
  const emotion = r.emotion
  const mappedEmotion = EMOTION_CONFIG_EMO[emotion] || '正常'

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

watch(
  () => role.value.emotion,
  async (newEmotion) => {
    const currentId = ++latestEmotionId
    await resolveAvatar()
    await nextTick()
    if (imageFadeRef.value) await imageFadeRef.value.waitForLoad()
    if (currentId !== latestEmotionId) return

    const config = EMOTION_CONFIG[newEmotion]
    if (!config) return

    if (config.animation && config.animation !== 'none')
      activeAnimationClass.value = config.animation

    if (config.bubbleImage && config.bubbleImage !== 'none') {
      // 修复代码：移除 ?t=... 形式的 cache-buster，避免由于本地重新加载导致的疯狂闪烁
      currentBubbleImageUrl.value = config.bubbleImage
      currentBubbleClass.value = config.bubbleClass

      if (bubbleTimeoutId !== null) {
        window.clearTimeout(bubbleTimeoutId)
        bubbleTimeoutId = null
      }

      if (!isBubbleVisible.value) {
        isBubbleVisible.value = true
      }

      bubbleTimeoutId = window.setTimeout(() => {
        isBubbleVisible.value = false
        bubbleTimeoutId = null
      }, 2000)
    }

    if (config.audio && config.audio !== 'none') {
      playBubbleAudio(config.audio)
    }
  },
  { immediate: true },
)

// 播放情绪气泡音效（音量跟随「气泡音量」设置，否则恒为满音量）
const playBubbleAudio = (src: string) => {
  if (!bubbleAudio.value) return
  bubbleAudio.value.volume = uiStore.bubbleVolume / 100
  bubbleAudio.value.src = src
  bubbleAudio.value.load()
  bubbleAudio.value.play().catch((e) => console.error('气泡音效播放失败:', e))
}

// 气泡音量设置变化时，对已加载的音效实时生效
watch(
  () => uiStore.bubbleVolume,
  (v) => {
    if (bubbleAudio.value) bubbleAudio.value.volume = v / 100
  },
)

// 思考中反馈：气泡 + 音效（由 currentStatus 驱动，与 emotion 解耦）
watch(
  () => gameStore.currentStatus,
  (newStatus) => {
    if (newStatus === 'thinking') {
      const config = EMOTION_CONFIG['AI思考']
      if (config && config.bubbleImage && config.bubbleImage !== 'none') {
        currentBubbleImageUrl.value = config.bubbleImage
        currentBubbleClass.value = config.bubbleClass

        if (bubbleTimeoutId !== null) {
          window.clearTimeout(bubbleTimeoutId)
          bubbleTimeoutId = null
        }
        if (!isBubbleVisible.value) {
          isBubbleVisible.value = true
        }
        bubbleTimeoutId = window.setTimeout(() => {
          isBubbleVisible.value = false
          bubbleTimeoutId = null
        }, 2000)
      }
      if (config?.audio && config.audio !== 'none') {
        playBubbleAudio(config.audio)
      }
    } else {
      // 离开思考态：隐藏思考气泡、停掉定时器
      isBubbleVisible.value = false
      if (bubbleTimeoutId !== null) {
        window.clearTimeout(bubbleTimeoutId)
        bubbleTimeoutId = null
      }
    }
  },
)
</script>

<style scoped>
.animate-breathing {
  animation: breathing 4s ease-in-out infinite alternate;
}

.animate-pulse-slow {
  animation: pulse-slow 3s cubic-bezier(0.4, 0, 0.6, 1) infinite;
}

@keyframes breathing {
  0% {
    transform: scale(1);
  }
  100% {
    transform: scale(1.02);
  }
}

@keyframes pulse-slow {
  0%,
  100% {
    opacity: 0.3;
  }
  50% {
    opacity: 1;
  }
}

.sweep-glow-ring {
  background: conic-gradient(
    from 0deg,
    transparent 40%,
    rgba(34, 211, 238, 0.1) 70%,
    rgba(34, 211, 238, 0.8) 100%
  );
  -webkit-mask: radial-gradient(transparent 68%, #000 69%);
  mask: radial-gradient(transparent 68%, #000 69%);
  animation: spin 4s linear infinite;
}

[data-tauri-drag-region] {
  -webkit-app-region: drag;
}
</style>
