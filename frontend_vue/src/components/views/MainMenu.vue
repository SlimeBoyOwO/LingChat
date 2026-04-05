<template>
  <div
    class="main-menu-page"
    :class="{ 'main-menu-page--panel-active': currentPage !== 'mainMenu' }"
  >
    <MainChat v-if="currentPage === 'gameMainView'" />
    <Settings v-else-if="currentPage === 'settings'" />
    <Save v-else-if="currentPage === 'save'" />

    <!-- 背景层（最底层） -->
    <div class="video-background" ref="bgRef"></div>

    <!-- 流星层（SVG动画） -->
    <div v-if="meteorsEnabled" class="meteor-wrapper">
      <svg id="meteor-svg" ref="svgRef">
        <defs>
          <linearGradient
            id="meteor-grad-fixed"
            gradientUnits="objectBoundingBox"
            x1="1"
            y1="0"
            x2="0"
            y2="0"
          >
            <stop offset="0%" stop-color="rgba(255,255,255,1)" />
            <stop offset="40%" stop-color="rgba(255,255,255,1)" />
            <stop offset="70%" stop-color="rgba(220,240,255,0.8)" />
            <stop offset="100%" stop-color="rgba(180,220,255,0)" />
          </linearGradient>
        </defs>
        <!-- Vue 响应式渲染流星路径 -->
        <path
          v-for="meteor in activeMeteors"
          :key="meteor.id"
          :d="meteor.d"
          class="meteor-path"
          :style="{ animationDuration: `${meteor.duration}s` }"
        />
      </svg>
    </div>

    <!-- 星星粒子层（位于背景和人物之间） -->
    <div v-if="starsEnabled" class="stars-layer" ref="starsLayerRef">
      <canvas id="stars-canvas" ref="canvasRef"></canvas>
    </div>

    <!-- 人物图层（位于星星之上，菜单之下） -->
    <img class="character-image" ref="charRef" src="../../assets/images/alona.png" alt="人物" />

    <!-- 菜单容器，绑定鼠标移动和移出事件实现视差 -->
    <div
      class="main-menu-page__container"
      v-if="currentPage === 'mainMenu'"
      ref="containerRef"
      @mousemove="handleMouseMove"
      @mouseleave="handleMouseLeave"
    >
      <!-- 主菜单 -->
      <Transition name="slide-left">
        <div class="main-menu-page__menu" v-if="menuState === 'main'">
          <MainMenuOptions
            @start-game="showGameModeMenu"
            @open-settings="handleContinueGame"
            @open-credits="handleOpenCredits"
          />
        </div>
      </Transition>

      <!-- 游戏模式菜单 -->
      <Transition name="slide-right">
        <div class="main-menu-page__menu" v-if="menuState === 'gameMode'">
          <GameModeOptions
            @back="backToMainMenu"
            @open-scripts="showScriptModeMenu"
            :loadingScripts="loadingScripts"
            :scripts="scripts"
          />
        </div>
      </Transition>

      <!-- 剧本模式菜单 -->
      <Transition name="slide-right">
        <div class="main-menu-page__menu" v-if="menuState === 'scriptMode'">
          <ScriptModeOptions @back="showGameModeMenu" :scripts="scripts" />
        </div>
      </Transition>

      <img
        src="../../assets/images/LingChatLogo.png"
        alt="LingChatLogo"
        class="main-menu-page__logo cursor-pointer hover:scale-105 transition-transform duration-300"
        @click="goToGithub"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue'
import { useRouter } from 'vue-router'
import { MainChat } from './'
import { SettingsPanel as Settings } from '../settings/'
import { MainMenuOptions, GameModeOptions } from './menu'
import { useUIStore } from '../../stores/modules/ui/ui'
import { useSettingsStore } from '../../stores/modules/settings'
import ScriptModeOptions from './menu/ScriptModeOptions.vue'
import { getScriptList, type ScriptSummary } from '@/api/services/script-info'
import { saveContinue } from '@/api/services/save'

const router = useRouter()
const uiStore = useUIStore()
const settingsStore = useSettingsStore()

// 页面与菜单状态
const currentPage = ref('mainMenu')
const menuState = ref<'main' | 'gameMode' | 'scriptMode'>('main')
const scripts = ref<ScriptSummary[]>([])
const loadingScripts = ref(false)
const starsEnabled = computed(() => settingsStore.mainMenuStarsEnabled)
const meteorsEnabled = computed(() => settingsStore.mainMenuMeteorsEnabled)

// DOM Refs
const containerRef = ref<HTMLElement | null>(null)
const bgRef = ref<HTMLElement | null>(null)
const charRef = ref<HTMLElement | null>(null)
const starsLayerRef = ref<HTMLElement | null>(null)
const canvasRef = ref<HTMLCanvasElement | null>(null)

const Save = Settings

/* ================== 菜单逻辑 ================== */
function showGameModeMenu() {
  menuState.value = 'gameMode'
}
function handleOpenCredits() {
  router.push('/credit')
}
function backToMainMenu() {
  menuState.value = 'main'
}
function showScriptModeMenu() {
  menuState.value = 'scriptMode'
}
function goToGithub() {
  window.open('https://github.com/SlimeBoyOwO/LingChat', '_blank')
}

const handleContinueGame = async () => {
  try {
    await saveContinue({ user_id: '1' })
    router.push('/chat')
  } catch (error) {
    alert('继续游戏失败，未创建存档或系统问题')
  }
}

function handleOpenSettings(tab?: string) {
  uiStore.toggleSettings(true)
  if (tab === 'save') {
    currentPage.value = 'save'
    uiStore.setSettingsTab('save')
  } else {
    currentPage.value = 'settings'
  }
}

watch(
  () => uiStore.showSettings,
  (newVal) => {
    if (!newVal && (currentPage.value === 'settings' || currentPage.value === 'save')) {
      currentPage.value = 'mainMenu'
      menuState.value = 'main'
    }
  },
)

/* ================== 视差倾斜 & 平移效果 (柔和版) ================== */
const PARALLAX_CONFIG = {
  MAX_ANGLE: 2.5, // [已调低] 最大倾斜角度，原为7，现改为2.5，更加轻微优雅
  VERTICAL_SCALE: 0.6, // 上下倾斜比例
  CHAR_MAX_SHIFT: 10, // [已调低] 人物平移减弱
  BG_MAX_SHIFT: 6, // [已调低] 背景平移减弱
  STARS_MAX_SHIFT: 20, // [已调低] 星星平移减弱
}

let targetOffsetX = 0
let targetOffsetY = 0
let currentOffsetX = 0
let currentOffsetY = 0
let parallaxRafId: number | null = null

function parallaxLoop() {
  currentOffsetX += (targetOffsetX - currentOffsetX) * 0.08 // 0.08更显顺滑阻尼
  currentOffsetY += (targetOffsetY - currentOffsetY) * 0.08

  if (charRef.value) {
    charRef.value.style.transform = `translate(-50%, -50%) translateX(${-currentOffsetX * PARALLAX_CONFIG.CHAR_MAX_SHIFT}px)`
  }
  if (bgRef.value) {
    bgRef.value.style.transform = `translateX(${-currentOffsetX * PARALLAX_CONFIG.BG_MAX_SHIFT}px)`
  }
  if (starsLayerRef.value) {
    starsLayerRef.value.style.transform = `translateX(${-currentOffsetX * PARALLAX_CONFIG.STARS_MAX_SHIFT}px)`
  }

  parallaxRafId = requestAnimationFrame(parallaxLoop)
}

function handleMouseMove(e: MouseEvent) {
  if (currentPage.value !== 'mainMenu') {
    targetOffsetX = 0
    targetOffsetY = 0
    return
  }
  const centerX = window.innerWidth / 2
  const centerY = window.innerHeight / 2
  targetOffsetX = (e.clientX - centerX) / centerX
  targetOffsetY = (e.clientY - centerY) / centerY
}

function handleMouseLeave() {
  targetOffsetX = 0
  targetOffsetY = 0
}

/* ================== 星星粒子系统 ================== */
interface Star {
  x: number
  y: number
  size: number
  baseOpacity: number
  rotation: number
  type: 'star' | 'circle'
}

const STARS_COUNT = 80
const FLICKER_SPEED = 0.003
const starsPositions = shallowRef<Star[]>([])
let starsFrameId: number | null = null
let starsCtx: CanvasRenderingContext2D | null = null

function drawStarShape(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  size: number,
  rotation: number,
) {
  ctx.save()
  ctx.translate(x, y)
  ctx.rotate(rotation)
  const gradient = ctx.createRadialGradient(0, 0, 0, 0, 0, size * 1.5)
  gradient.addColorStop(0, '#ffffff')
  gradient.addColorStop(0.6, 'rgba(255, 240, 200, 0.9)')
  gradient.addColorStop(1, 'rgba(255, 200, 100, 0)')
  ctx.shadowColor = 'rgba(255, 255, 200, 0.8)'
  ctx.shadowBlur = 20
  ctx.fillStyle = gradient
  ctx.beginPath()
  for (let i = 0; i < 8; i++) {
    const angle = (i * Math.PI) / 4
    const radius = i % 2 === 0 ? size : size * 0.4
    const dx = Math.cos(angle) * radius
    const dy = Math.sin(angle) * radius
    if (i === 0) ctx.moveTo(dx, dy)
    else ctx.lineTo(dx, dy)
  }
  ctx.closePath()
  ctx.fill()
  ctx.restore()
}

function drawCircleShape(ctx: CanvasRenderingContext2D, x: number, y: number, size: number) {
  ctx.save()
  const gradient = ctx.createRadialGradient(x, y, 0, x, y, size * 1.5)
  gradient.addColorStop(0, '#ffffff')
  gradient.addColorStop(0.6, 'rgba(255, 240, 200, 0.9)')
  gradient.addColorStop(1, 'rgba(255, 200, 100, 0)')
  ctx.shadowColor = 'rgba(255, 255, 200, 0.8)'
  ctx.shadowBlur = 20
  ctx.fillStyle = gradient
  ctx.beginPath()
  ctx.arc(x, y, size, 0, Math.PI * 2)
  ctx.fill()
  ctx.restore()
}

function generateStars() {
  if (!canvasRef.value) return
  const w = window.innerWidth
  const h = window.innerHeight
  canvasRef.value.width = w
  canvasRef.value.height = h
  starsCtx = canvasRef.value.getContext('2d')

  const tempStars: Star[] = []
  for (let i = 0; i < STARS_COUNT; i++) {
    tempStars.push({
      x: Math.random() * w,
      y: Math.random() * (h * 0.33),
      size: Math.random() * 5 + 2,
      baseOpacity: Math.random() * 0.5 + 0.5,
      rotation: Math.random() * Math.PI * 2,
      type: Math.random() > 0.2 ? 'star' : 'circle',
    })
  }
  starsPositions.value = tempStars
}

function renderStars() {
  if (!starsCtx || !canvasRef.value) return
  const w = canvasRef.value.width
  const h = canvasRef.value.height

  // 关键修复：重置全局透明度再 clearRect，避免清除不干净导致的问题
  starsCtx.globalAlpha = 1.0
  starsCtx.clearRect(0, 0, w, h)

  const now = Date.now()
  starsPositions.value.forEach((pos) => {
    const flicker = 0.6 + 0.4 * Math.sin(now * FLICKER_SPEED + pos.x)
    const opacity = Math.min(pos.baseOpacity * flicker, 1.0)
    starsCtx!.globalAlpha = opacity

    if (pos.type === 'star') {
      drawStarShape(starsCtx!, pos.x, pos.y, pos.size, pos.rotation)
    } else {
      drawCircleShape(starsCtx!, pos.x, pos.y, pos.size)
    }
  })
}

function flickerAnimation() {
  renderStars()
  starsFrameId = requestAnimationFrame(flickerAnimation)
}

function handleResize() {
  stopStars()
  startStars()
}

/* ================== 流星雨系统 ================== */
interface MeteorData {
  id: number
  d: string
  duration: number
  startY: number
}
const activeMeteors = ref<MeteorData[]>([])
let meteorIdCounter = 0
let meteorIntervalId: ReturnType<typeof setInterval> | null = null
const METEOR_CONFIG = {
  MAX_COUNT: 3,
  GENERATE_INTERVAL: 1000,
  MIN_DISTANCE: 800,
  DURATION_MIN: 9,
  DURATION_MAX: 14,
  START_X_MIN: 800,
  START_X_MAX: 1200,
  START_Y_RANGE: 0.25,
  END_X_MIN: -200,
  END_X_MAX: -400,
  END_Y_OFFSET: 1,
  CONTROL_X_RATIO_MIN: 0.5,
  CONTROL_X_RATIO_MAX: 0.9,
  CONTROL_Y_OFFSET: -0.1,
}

function getValidStartY() {
  const h = window.innerHeight
  const maxAttempts = 120
  let attempts = 0
  while (attempts < maxAttempts) {
    const startY = -Math.random() * (h * METEOR_CONFIG.START_Y_RANGE)
    const tooClose = activeMeteors.value.some(
      (m) => Math.abs(startY - m.startY) < METEOR_CONFIG.MIN_DISTANCE,
    )
    if (!tooClose) return startY
    attempts++
  }
  return -Math.random() * (h * METEOR_CONFIG.START_Y_RANGE)
}

function createMeteor() {
  const w = window.innerWidth
  const h = window.innerHeight
  const startY = getValidStartY()
  const startX =
    w +
    METEOR_CONFIG.START_X_MIN +
    Math.random() * (METEOR_CONFIG.START_X_MAX - METEOR_CONFIG.START_X_MIN)
  const endX =
    METEOR_CONFIG.END_X_MIN + Math.random() * (METEOR_CONFIG.END_X_MAX - METEOR_CONFIG.END_X_MIN)
  const endY = startY + h * METEOR_CONFIG.END_Y_OFFSET
  const controlX =
    w *
    (METEOR_CONFIG.CONTROL_X_RATIO_MIN +
      Math.random() * (METEOR_CONFIG.CONTROL_X_RATIO_MAX - METEOR_CONFIG.CONTROL_X_RATIO_MIN))
  const controlY = startY + h * METEOR_CONFIG.CONTROL_Y_OFFSET
  const duration =
    METEOR_CONFIG.DURATION_MIN +
    Math.random() * (METEOR_CONFIG.DURATION_MAX - METEOR_CONFIG.DURATION_MIN)
  const d = `M ${startX} ${startY} Q ${controlX} ${controlY} ${endX} ${endY}`
  const id = meteorIdCounter++

  activeMeteors.value.push({ id, d, duration, startY })
  setTimeout(() => {
    activeMeteors.value = activeMeteors.value.filter((m) => m.id !== id)
  }, duration * 1000)
}

function updateMeteorShower() {
  if (activeMeteors.value.length < METEOR_CONFIG.MAX_COUNT) createMeteor()
}

function startMeteorShower() {
  if (meteorIntervalId) clearInterval(meteorIntervalId)
  for (let i = 0; i < 3; i++) {
    setTimeout(() => {
      if (activeMeteors.value.length < METEOR_CONFIG.MAX_COUNT) createMeteor()
    }, i * 300)
  }
  meteorIntervalId = setInterval(updateMeteorShower, METEOR_CONFIG.GENERATE_INTERVAL)
}

function startStars() {
  if (!canvasRef.value) return
  generateStars()
  flickerAnimation()
  window.addEventListener('resize', handleResize)
}

function stopStars() {
  window.removeEventListener('resize', handleResize)
  if (starsFrameId) {
    cancelAnimationFrame(starsFrameId)
    starsFrameId = null
  }
  if (starsCtx && canvasRef.value) {
    starsCtx.clearRect(0, 0, canvasRef.value.width, canvasRef.value.height)
  }
}

function stopMeteorShower() {
  if (meteorIntervalId) {
    clearInterval(meteorIntervalId)
    meteorIntervalId = null
  }
  activeMeteors.value = []
}

/* ================== 生命周期Hooks ================== */

// 抽取接口请求逻辑，不阻塞动画初始化
async function fetchScripts() {
  loadingScripts.value = true
  try {
    scripts.value = await getScriptList()
  } catch (e) {
    uiStore.showError({
      errorCode: 'script_list_failed',
      message: '获取剧本列表失败：请确认后端已启动',
    })
    scripts.value = []
  } finally {
    loadingScripts.value = false
  }
}

onMounted(() => {
  const initializeMenu = async () => {
    if (starsEnabled.value || meteorsEnabled.value) {
      uiStore.showInfo({
        title: 'Tip',
        message: '如果你觉得在这个页面很卡，可以前往 通用设置 中关闭星星粒子或流星动画。',
        duration: 5000,
      })
    }

    parallaxLoop()
    fetchScripts()
  }

  initializeMenu()

  watch(
    starsEnabled,
    async (enabled) => {
      if (enabled) {
        await nextTick()
        startStars()
      } else {
        stopStars()
      }
    },
    { immediate: true },
  )

  watch(
    meteorsEnabled,
    (enabled) => {
      if (enabled) {
        startMeteorShower()
      } else {
        stopMeteorShower()
      }
    },
    { immediate: true },
  )
})

onUnmounted(() => {
  stopStars()
  stopMeteorShower()
  if (parallaxRafId) cancelAnimationFrame(parallaxRafId)
})
</script>

<style scoped>
@font-face {
  font-family: 'Maoken Assorted Sans';
  src: url('./assets/fonts/MaokenAssortedSans.ttf') format('truetype');
  font-weight: normal;
  font-style: normal;
  font-display: swap;
}

.main-menu-page {
  width: 100%;
  height: 100%;
  position: relative;
  overflow: hidden;
}

.main-menu-page--panel-active::before {
  content: '';
  position: absolute;
  inset: 0;
  backdrop-filter: blur(12px) brightness(0.9);
  z-index: 10;
  pointer-events: none;
}

/* 菜单容器 */
.main-menu-page__container {
  width: 100%;
  height: 100%;
  display: flex;
  justify-content: flex-start;
  align-items: center;
  position: absolute; /* 确保它覆盖全屏叠加在顶层 */
  top: 0;
  left: 0;
  transform-style: preserve-3d;
  will-change: transform;
  z-index: 3;
}

.main-menu-page__menu {
  display: flex;
  flex-direction: column;
  padding: 20px;
  margin-left: 10vw;
  position: absolute;
  z-index: 5;
}

.main-menu-page__logo {
  position: absolute;
  top: 5vh;
  right: 5vw;
  height: auto;
  width: auto;
  max-width: 40vw;
  filter: drop-shadow(0 4px 8px rgba(0, 0, 0, 0.3));
  z-index: 5;
}

/* 页面切换动画 */
.slide-left-enter-active,
.slide-left-leave-active,
.slide-right-enter-active,
.slide-right-leave-active {
  transition: all 0.4s cubic-bezier(0.7, 0, 0.2, 1);
}

.slide-left-enter-from,
.slide-left-leave-to {
  transform: translateX(-120%);
  opacity: 0;
}

.slide-right-enter-from,
.slide-right-leave-to {
  transform: translateX(120%);
  opacity: 0;
}

/* ========== 背景层 ========== */
.video-background {
  position: absolute;
  top: 0;
  left: -10%;
  width: 120%;
  height: 100%;
  background-image: url('../../assets/images/background2.png');
  background-size: cover;
  background-position: center;
  z-index: -2;
  /* 移除 transition */
  will-change: transform;
}

/* ========== 人物图层 ========== */
.character-image {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  max-width: 100%;
  max-height: 100%;
  z-index: 3;
  pointer-events: none;
  /* 移除 transition */
  will-change: transform;
}

/* ========== 星星粒子层 ========== */
.stars-layer {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: 1;
  pointer-events: none;
  overflow: hidden;
  /* 移除 transition */
  will-change: transform;
}

#stars-canvas {
  width: 100%;
  height: 100%;
  display: block;
}

/* ========== 流星层 ========== */
.meteor-wrapper {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  z-index: 2;
  pointer-events: none;
  overflow: hidden;
}

#meteor-svg {
  width: 100%;
  height: 100%;
}

.meteor-path {
  fill: none;
  stroke: url(#meteor-grad-fixed);
  stroke-width: 5;
  stroke-linecap: round;
  filter: drop-shadow(0 0 20px rgba(255, 255, 255, 0.95))
    drop-shadow(0 0 12px rgba(180, 220, 255, 0.9));
  stroke-dasharray: 600, 3000;
  stroke-dashoffset: 3000;
  animation-name: meteor-move;
  animation-timing-function: linear;
  animation-fill-mode: forwards;
}

@keyframes meteor-move {
  0% {
    stroke-dashoffset: 3000;
  }
  100% {
    stroke-dashoffset: -3000;
  }
}
</style>
