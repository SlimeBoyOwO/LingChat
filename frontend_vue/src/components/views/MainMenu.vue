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
    <MeteorAnimation :meteors-enabled="meteorsEnabled" />

    <!-- 星星粒子层（位于背景和人物之间） -->
    <StarAnimation :stars-enabled="starsEnabled" :stars-layer-ref="starsLayerRef" />

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
import MeteorAnimation from '../game/standard/animations/MeteorAnimation.vue'
import StarAnimation from '../game/standard/animations/StarAnimation.vue'

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

/* ================== 视差倾斜 & 平移效果 (优化版) ================== */
const PARALLAX_CONFIG = {
  MAX_ANGLE: 2.5, // [已调低] 最大倾斜角度，原为7，现改为2.5，更加轻微优雅
  VERTICAL_SCALE: 0.6, // 上下倾斜比例
  CHAR_MAX_SHIFT: 10, // [已调低] 人物平移减弱
  BG_MAX_SHIFT: 6, // [已调低] 背景平移减弱
  STARS_MAX_SHIFT: 20, // [已调低] 星星平移减弱
  DAMPING: 0.08, // 阻尼系数
  IDLE_THRESHOLD: 0.01, // 空闲阈值，低于此值停止动画
}

let targetOffsetX = 0
let targetOffsetY = 0
let currentOffsetX = 0
let currentOffsetY = 0
let parallaxRafId: number | null = null
let isParallaxRunning = false

/**
 * 优化的视差动画循环
 * - 只在有实际移动时启动
 * - 当值收敛到目标时自动停止
 * - 避免空闲时的不必要计算
 */
function parallaxLoop() {
  const deltaX = targetOffsetX - currentOffsetX
  const deltaY = targetOffsetY - currentOffsetY

  // 检查是否已经收敛到目标值
  const hasConverged =
    Math.abs(deltaX) < PARALLAX_CONFIG.IDLE_THRESHOLD &&
    Math.abs(deltaY) < PARALLAX_CONFIG.IDLE_THRESHOLD

  if (hasConverged) {
    // 动画已收敛到目标值，直接对齐并停止循环
    currentOffsetX = targetOffsetX
    currentOffsetY = targetOffsetY
    applyParallaxTransforms(currentOffsetX, currentOffsetY)
    isParallaxRunning = false
    parallaxRafId = null
    return
  }

  // 应用阻尼插值
  currentOffsetX += deltaX * PARALLAX_CONFIG.DAMPING
  currentOffsetY += deltaY * PARALLAX_CONFIG.DAMPING

  applyParallaxTransforms(currentOffsetX, currentOffsetY)

  parallaxRafId = requestAnimationFrame(parallaxLoop)
}

/**
 * 应用视差变换到各层元素
 */
function applyParallaxTransforms(offsetX: number, offsetY: number) {
  const charShift = -offsetX * PARALLAX_CONFIG.CHAR_MAX_SHIFT
  const bgShift = -offsetX * PARALLAX_CONFIG.BG_MAX_SHIFT
  const starsShift = -offsetX * PARALLAX_CONFIG.STARS_MAX_SHIFT

  if (charRef.value) {
    charRef.value.style.transform = `translate(-50%, -50%) translateX(${charShift}px)`
  }
  if (bgRef.value) {
    bgRef.value.style.transform = `translateX(${bgShift}px)`
  }
  if (starsLayerRef.value) {
    starsLayerRef.value.style.transform = `translateX(${starsShift}px)`
  }
}

/**
 * 启动视差动画（如果尚未运行）
 */
function startParallaxIfNeeded() {
  if (!isParallaxRunning) {
    isParallaxRunning = true
    parallaxLoop()
  }
}

function handleMouseMove(e: MouseEvent) {
  if (currentPage.value !== 'mainMenu') {
    targetOffsetX = 0
    targetOffsetY = 0
    startParallaxIfNeeded()
    return
  }
  const centerX = window.innerWidth / 2
  const centerY = window.innerHeight / 2
  targetOffsetX = (e.clientX - centerX) / centerX
  targetOffsetY = (e.clientY - centerY) / centerY
  startParallaxIfNeeded()
}

function handleMouseLeave() {
  targetOffsetX = 0
  targetOffsetY = 0
  startParallaxIfNeeded()
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
    // 性能提示只显示一次
    const PERFORMANCE_TIP_KEY = 'mainMenuPerformanceTipShown'
    if (
      (starsEnabled.value || meteorsEnabled.value) &&
      !localStorage.getItem(PERFORMANCE_TIP_KEY)
    ) {
      localStorage.setItem(PERFORMANCE_TIP_KEY, 'true')
      uiStore.showInfo({
        title: 'Tip',
        message: '如果你觉得在这个页面很卡，可以前往 通用设置 中关闭星星粒子或流星动画。',
        duration: 5000,
      })
    }

    fetchScripts()
  }

  initializeMenu()
})

onUnmounted(() => {
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
</style>
