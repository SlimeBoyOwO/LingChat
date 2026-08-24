<template>
  <div
    class="main-menu-page"
    :class="[{ 'main-menu-page--panel-active': currentPage !== 'mainMenu' }, menuThemeClass]"
  >
    <MainChat v-if="currentPage === 'gameMainView'" />
    <Settings v-else-if="currentPage === 'settings'" />
    <Save v-else-if="currentPage === 'save'" />

    <!-- 背景层（最底层） -->
    <div
      class="video-background"
      ref="bgRef"
    ></div>

    <!-- 剧本可持久化的预设式标题异常层：只渲染纯文本/CSS，不接受 HTML 或资源路径。 -->
    <div
      v-if="menuEffect.theme !== 'normal'"
      class="script-menu-effect"
      aria-hidden="true"
    >
      <div class="script-menu-effect__scanlines"></div>
      <div
        v-if="menuEffect.message"
        class="script-menu-effect__message"
      >
        {{ menuEffect.message }}
      </div>
    </div>

    <!-- 流星层（SVG动画） -->
    <MeteorAnimation
      :meteors-enabled="visualMeteorsEnabled"
      :meteor-fps="meteorFps"
    />

    <!-- 星星粒子层（位于背景和人物之间） -->
    <StarAnimation
      :stars-enabled="visualStarsEnabled"
      :stars-layer-ref="starsLayerRef"
      :stars-fps="starsFps"
    />

    <!-- 人物图层（位于星星之上，菜单之下） -->
    <img
      class="character-image"
      ref="charRef"
      src="../../assets/images/alona.png"
      :alt="$t('views.mainMenu.characterAlt')"
    />

    <!-- 菜单容器，绑定鼠标移动和移出事件实现视差 -->
    <StartPage
      v-if="currentPage === 'mainMenu'"
      ref="containerRef"
      @mousemove="handleMouseMove"
      @mouseleave="handleMouseLeave"
    >
      <!-- 主菜单 -->
      <Transition name="slide-left">
        <MainMenuOptions
          v-if="menuState === 'main'"
          @start-game="showGameModeMenu"
          @open-settings="handleOpenSettings"
          @open-credits="handleOpenCredits"
          @open-workshop="showWorkshopMenu"
          @open-script-editor="() => router.push('/script-editor')"
        />
      </Transition>

      <!-- 游戏模式菜单 -->
      <Transition name="slide-right">
        <GameModeOptions
          v-if="menuState === 'gameMode'"
          @back="backToMainMenu"
          @open-scripts="showScriptModeMenu"
          :loadingScripts="loadingScripts"
          :scripts="scripts"
        />
      </Transition>

      <!-- 剧本模式菜单 -->
      <Transition name="slide-right">
        <ScriptModeOptions
          v-if="menuState === 'scriptMode'"
          @back="showGameModeMenu"
          @script-state-reset="fetchScriptMenuEffect"
          :scripts="scripts"
        />
      </Transition>

      <!-- 创意工坊菜单 -->
      <Transition name="slide-right">
        <WorkshopOptions
          v-if="menuState === 'workshop'"
          @back="backToMainMenu"
          :scripts="scripts"
        />
      </Transition>

      <StartLogo
        :corrupted="menuEffect.theme !== 'normal'"
        @click="goToGithub"
      />
    </StartPage>

    <!-- DLC 识别提示（右下角小字；有已识别 DLC 时才显示） -->
    <div
      v-if="currentPage === 'mainMenu' && dlcNames.length > 0"
      class="dlc-hint"
    >
      {{ $t('views.mainMenu.dlcRecognized', { names: dlcNames.join('、') }) }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { StartLogo, StartPage } from './menu/base'
import { WorkshopOptions, GameModeOptions, MainMenuOptions, ScriptModeOptions } from './menu/page'
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import MainChat from './MainChat.vue'
import { SettingsPanel as Settings } from '../settings/'
import { useUIStore } from '../../stores/modules/ui/ui'
import { useSettingsStore } from '../../stores/modules/settings'
import { getScriptList, type ScriptSummary } from '@/api/services/script-info'
import { listDlcs } from '@/api/services/dlc'
import { invoke } from '@tauri-apps/api/core'
import { useGameStore } from '../../stores/modules/game'
import { applyWebInitData } from '../../stores/modules/game/actions'
import type { WebInitData } from '@/api/services/game-info'
import MeteorAnimation from '../game/standard/animations/MeteorAnimation.vue'
import StarAnimation from '../game/standard/animations/StarAnimation.vue'
import { useParallaxAnimation } from '../game/standard/animations/ParallaxAnimation'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
const router = useRouter()
const uiStore = useUIStore()
const settingsStore = useSettingsStore()

// 页面与菜单状态
const currentPage = ref('mainMenu')
const menuState = ref<'main' | 'gameMode' | 'scriptMode' | 'workshop'>('main')
const scripts = ref<ScriptSummary[]>([])
const loadingScripts = ref(false)
// 已识别的 DLC 剧本包名（主菜单右下角提示用）
const dlcNames = ref<string[]>([])
type ScriptMenuTheme = 'normal' | 'blood' | 'ghost'
interface ScriptMenuEffect {
  theme: ScriptMenuTheme
  message?: string
  /** 特效归属剧本的 path_key（如 standalone/第七个测试剧本） */
  owner?: string
}
const menuEffect = ref<ScriptMenuEffect>({ theme: 'normal' })
const starsEnabled = computed(() => settingsStore.mainMenuStarsEnabled)
const meteorsEnabled = computed(() => settingsStore.mainMenuMeteorsEnabled)
const visualStarsEnabled = computed(() => starsEnabled.value && menuEffect.value.theme === 'normal')
const visualMeteorsEnabled = computed(
  () => meteorsEnabled.value && menuEffect.value.theme === 'normal',
)
const menuThemeClass = computed(() => `main-menu-page--${menuEffect.value.theme}`)
const meteorFps = computed(() => settingsStore.meteorFps)
const starsFps = computed(() => settingsStore.starsFps)

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
function showWorkshopMenu() {
  menuState.value = 'workshop'
}
function goToGithub() {
  window.open('https://github.com/SlimeBoyOwO/LingChat', '_blank')
}

const handleContinueGame = async () => {
  try {
    const { saves } = await invoke<{ saves: Array<{ id: number }>; total: number }>('list_saves', {
      page: 1,
      pageSize: 1,
    })
    if (!saves || saves.length === 0) {
      uiStore.showWarning({
        title: t('views.mainMenu.noSaveTitle'),
        message: t('views.mainMenu.noSaveMessage'),
      })
      return
    }
    const gameInfo = await invoke<WebInitData>('load_save', { saveId: saves[0].id })
    const gameStore = useGameStore()
    applyWebInitData(gameStore.$state, gameInfo)
    router.push('/chat')
  } catch (error) {
    console.error('继续游戏失败:', error)
    uiStore.showError({
      title: t('views.mainMenu.continueFailTitle'),
      message: t('views.mainMenu.continueFailMessage'),
    })
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
      message: t('views.mainMenu.scriptListFailed'),
    })
    scripts.value = []
  } finally {
    loadingScripts.value = false
  }
}

// DLC 识别提示：读取已安装的 DLC 剧本包（失败静默，只是个小字提示）
async function fetchDlcs() {
  try {
    dlcNames.value = (await listDlcs()).map((d) => d.name)
  } catch {
    dlcNames.value = []
  }
}

async function fetchScriptMenuEffect() {
  try {
    const effect = await invoke<ScriptMenuEffect | null>('get_script_menu_effect')
    if (effect && ['blood', 'ghost'].includes(effect.theme)) {
      menuEffect.value = effect
    } else {
      menuEffect.value = { theme: 'normal' }
    }
  } catch {
    // Invalid/missing state must always fail open to the ordinary accessible menu.
    menuEffect.value = { theme: 'normal' }
  }
}

// DLC 管理页导入/卸载后，剧本列表与提示一并刷新
watch(
  () => uiStore.dlcRefreshToken,
  () => {
    fetchScripts()
    fetchDlcs()
    fetchScriptMenuEffect()
  },
)

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
        message: t('views.mainMenu.perfTip'),
        duration: 5000,
      })
    }

    fetchScripts()
    fetchDlcs()
    fetchScriptMenuEffect()
  }

  initializeMenu()
})
</script>

<style scoped>
@font-face {
  font-family: 'Maoken Assorted Sans';
  src: url('/fonts/MaokenAssortedSans.woff2') format('woff2');
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

/* DLC 识别提示：右下角半透明黄小字，不挡菜单 */
.dlc-hint {
  position: absolute;
  right: 14px;
  bottom: 30px;
  z-index: 5;
  font-size: 12px;
  letter-spacing: 0.05em;
  color: rgba(255, 214, 90, 0.78);
  text-shadow:
    0 1px 4px rgba(0, 0, 0, 0.7),
    0 0 2px rgba(0, 0, 0, 0.45);
  pointer-events: none;
}

/* 菜单容器 */

/* 页面切换动画 */
.slide-left-enter-active,
.slide-left-leave-active,
.slide-right-enter-active,
.slide-right-leave-active {
  transition: all 0.4s cubic-bezier(0.7, 0, 0.2, 1);
}

/* Remove leaving elements from flex flow immediately to prevent layout jump */
.slide-left-leave-active,
.slide-right-leave-active {
  position: absolute;
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

/* ========== 剧本驱动的标题异常预设 ========== */
.script-menu-effect {
  position: absolute;
  inset: 0;
  z-index: 7;
  pointer-events: none;
  overflow: hidden;
}

.script-menu-effect::before {
  content: '';
  position: absolute;
  inset: 0;
  background:
    radial-gradient(circle at 50% 55%, transparent 0 28%, rgba(20, 0, 0, 0.44) 80%),
    repeating-linear-gradient(90deg, transparent 0 43px, rgba(255, 255, 255, 0.018) 44px 45px);
  mix-blend-mode: multiply;
}

.script-menu-effect__scanlines {
  position: absolute;
  inset: -12%;
  opacity: 0.3;
  background: repeating-linear-gradient(
    0deg,
    transparent 0 2px,
    rgba(0, 0, 0, 0.45) 3px,
    rgba(255, 255, 255, 0.025) 4px
  );
  animation: menu-scan 7s linear infinite;
}

.script-menu-effect__message {
  position: absolute;
  top: 18px;
  left: 20px;
  max-width: min(620px, 70vw);
  color: rgba(255, 214, 214, 0.92);
  font:
    600 13px/1.45 Consolas,
    'Cascadia Mono',
    monospace;
  letter-spacing: 0.12em;
  white-space: pre-wrap;
  text-shadow:
    -2px 0 rgba(255, 0, 0, 0.75),
    2px 0 rgba(0, 180, 255, 0.55),
    0 0 8px rgba(255, 0, 0, 0.8);
  animation: menu-message-glitch 2.8s steps(1, end) infinite;
}

.main-menu-page--blood {
  background: #090000;
}

.main-menu-page--blood .video-background {
  filter: grayscale(0.76) sepia(0.86) hue-rotate(315deg) saturate(3.4) brightness(0.46)
    contrast(1.45);
}

.main-menu-page--blood .character-image {
  opacity: 0.68;
  filter: grayscale(0.8) sepia(0.9) hue-rotate(315deg) saturate(2.5) contrast(1.45)
    drop-shadow(8px 0 0 rgba(150, 0, 0, 0.22));
}

.main-menu-page--blood .script-menu-effect {
  background:
    linear-gradient(110deg, rgba(75, 0, 0, 0.34), transparent 42%),
    radial-gradient(circle at 50% 50%, transparent 20%, rgba(95, 0, 0, 0.4));
}

.main-menu-page--blood :deep(.start-item) {
  color: #ffd8d8 !important;
  text-shadow:
    2px 0 rgba(120, 0, 0, 0.8),
    -1px 0 rgba(0, 110, 130, 0.65) !important;
}

.main-menu-page--ghost {
  background: #090b0d;
}

.main-menu-page--ghost .video-background {
  filter: grayscale(1) brightness(0.42) contrast(1.35);
}

.main-menu-page--ghost .character-image {
  opacity: 0.2;
  filter: grayscale(1) contrast(1.5) blur(0.5px);
  animation: ghost-character 5.5s steps(1, end) infinite;
}

.main-menu-page--ghost .script-menu-effect {
  background: radial-gradient(circle at 50% 45%, transparent 20%, rgba(220, 230, 235, 0.11));
}

.main-menu-page--ghost .script-menu-effect__message {
  color: rgba(225, 235, 238, 0.84);
  text-shadow:
    -2px 0 rgba(255, 255, 255, 0.5),
    2px 0 rgba(20, 20, 20, 0.9);
}

@keyframes menu-scan {
  from {
    transform: translateY(-8%);
  }
  to {
    transform: translateY(8%);
  }
}

@keyframes menu-message-glitch {
  0%,
  90%,
  100% {
    transform: translate(0);
    opacity: 0.92;
  }
  91% {
    transform: translate(-3px, 1px);
    opacity: 0.55;
  }
  93% {
    transform: translate(4px, -1px);
    opacity: 1;
  }
  95% {
    transform: translate(0);
  }
}

@keyframes ghost-character {
  0%,
  86%,
  100% {
    transform: translate(-50%, -50%);
  }
  87% {
    transform: translate(calc(-50% - 5px), -50%);
  }
  89% {
    transform: translate(calc(-50% + 7px), calc(-50% + 2px));
  }
  91% {
    transform: translate(-50%, -50%);
  }
}

@media (prefers-reduced-motion: reduce) {
  .script-menu-effect__scanlines,
  .script-menu-effect__message,
  .main-menu-page--ghost .character-image {
    animation: none !important;
  }
}
</style>
