<template>
  <div
    class="main-menu-page"
    :class="{ 'main-menu-page--panel-active': currentPage !== 'mainMenu' }"
  >
    <MainChat v-if="currentPage === 'gameMainView'" />
    <Settings v-else-if="currentPage === 'settings'" />
    <Save v-else-if="currentPage === 'save'" />

    <!-- 菜单容器 -->
    <div class="main-menu-page__container" v-if="currentPage === 'mainMenu'">
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
        class="main-menu-page__logo"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { MainChat } from './'
import { SettingsPanel as Settings } from '../settings/'
import { MainMenuOptions, GameModeOptions } from './menu'
import { useUIStore } from '../../stores/modules/ui/ui'
import ScriptModeOptions from './menu/ScriptModeOptions.vue'
import { getScriptList, type ScriptSummary } from '@/api/services/script-info'

const router = useRouter()

// 页面状态
const currentPage = ref('mainMenu')

// 菜单状态：main（主菜单）或 gameMode（游戏模式选择）
const menuState = ref<'main' | 'gameMode' | 'scriptMode'>('main')

const scripts = ref<ScriptSummary[]>([])
const loadingScripts = ref(false)

const uiStore = useUIStore()

// 显示游戏模式菜单
function showGameModeMenu() {
  menuState.value = 'gameMode'
}

function handleOpenCredits() {
  router.push('/credit')
}

// 返回主菜单
function backToMainMenu() {
  menuState.value = 'main'
}

// 显示剧本模式菜单
function showScriptModeMenu() {
  menuState.value = 'scriptMode'
}

// 处理设置面板打开
function handleOpenSettings(tab?: string) {
  uiStore.toggleSettings(true)
  if (tab === 'save') {
    currentPage.value = 'save'
    uiStore.setSettingsTab('save')
  } else {
    currentPage.value = 'settings'
  }
}

// 监听设置面板关闭，返回主菜单
watch(
  () => uiStore.showSettings,
  (newVal) => {
    if (!newVal && (currentPage.value === 'settings' || currentPage.value === 'save')) {
      currentPage.value = 'mainMenu'
      menuState.value = 'main'
    }
  },
)

onMounted(async () => {
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
})

// Save 组件占位（如果不存在则需要创建或使用 Settings）
const Save = Settings
</script>

<style scoped>
/* 定义自定义字体 */
@font-face {
  font-family: 'Maoken Assorted Sans';
  src: url('./assets/fonts/MaokenAssortedSans.ttf') format('truetype');
  font-weight: normal;
  font-style: normal;
  font-display: swap;
}

/* 主页面容器 - 使用具体类名避免冲突 */
.main-menu-page {
  width: 100%;
  height: 100%;
  position: relative;
}

.main-menu-page::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background-image: url('../../assets/images/background.png');
  background-size: cover;
  background-position: center;
  z-index: 0;
  filter: blur(0px) brightness(1);
  transition: filter 0.6s cubic-bezier(0.7, 0, 0.2, 1);
  pointer-events: none;
}

.main-menu-page--panel-active::before {
  filter: blur(12px) brightness(0.9);
}

/* 菜单容器 */
.main-menu-page__container {
  width: 100%;
  height: 100%;
  display: flex;
  justify-content: flex-start;
  align-items: center;
  position: relative;
}

/* 菜单位置 */
.main-menu-page__menu {
  display: flex;
  flex-direction: column;
  padding: 20px;
  margin-left: 10vw;
  position: absolute;
  z-index: 1;
}

/* Logo */
.main-menu-page__logo {
  position: absolute;
  top: 5vh;
  right: 5vw;
  height: 40vh;
  width: auto;
  max-width: 40vw;
  filter: drop-shadow(0 4px 8px rgba(0, 0, 0, 0.3));
  z-index: 1;
}

/* 向左滑出动画 */
.slide-left-enter-active,
.slide-left-leave-active {
  transition: all 0.4s cubic-bezier(0.7, 0, 0.2, 1);
}

.slide-left-enter-from,
.slide-left-leave-to {
  transform: translateX(-120%);
  opacity: 0;
}

/* 从右滑入动画 */
.slide-right-enter-active,
.slide-right-leave-active {
  transition: all 0.4s cubic-bezier(0.7, 0, 0.2, 1);
}

.slide-right-enter-from,
.slide-right-leave-to {
  transform: translateX(120%);
  opacity: 0;
}
</style>
