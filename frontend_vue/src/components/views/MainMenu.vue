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
            @open-settings="handleOpenSettings"
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
import { computed, onMounted, ref, watch } from 'vue'
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
import { useParallaxAnimation } from '../game/standard/animations/ParallaxAnimation'

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

/* ================== 视差动画 Hook ================== */
const { handleMouseMove, handleMouseLeave } = useParallaxAnimation({
  charRef,
  bgRef,
  starsLayerRef,
})

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
