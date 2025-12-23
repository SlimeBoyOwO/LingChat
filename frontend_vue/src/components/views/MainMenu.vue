<template>
  <div :class="['body', { 'panel-active': currentPage !== 'mainMenu' }]">
    <Loader :loading="loading" :progress="progress" />
    <MainChat v-if="currentPage === 'gameMainView'" />
    <Settings v-else-if="currentPage === 'settings'" />
    <Save v-else-if="currentPage === 'save'" />
    <Transition name="main-menu-animation" :duration="300">
      <div class="menu-container" v-if="currentPage === 'mainMenu'">
        <div class="main-menu">
          <template v-for="item in menuItems">
            <button
              class="menu-item"
              v-if="item.visibility.value"
              :key="item.order"
              @click="item.action"
            >
              {{ item.label }}
            </button>
          </template>
        </div>
        <img src="../../assets/images/LingChatLogo.png" alt="LingChatLogo" class="logo" />
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { Loader, MainChat } from './'
import { SettingsPanel as Settings } from '../settings/'
import { useRouter } from 'vue-router'
const currentPage = ref('mainMenu')
const loading = ref(true)
const progress = ref(0)
const menuItems = [
  { order: 0, label: '继续游戏', action: continueGame, visibility: ref(false) }, //TODO:只在有存档的时候显示
  { order: 1, label: '开始游戏', action: newGame, visibility: ref(true) },
  { order: 2, label: '存档', action: openSave, visibility: ref(true) },
  { order: 3, label: '设置', action: openSettings, visibility: ref(true) },
  {
    order: 4,
    label: '致谢名单',
    action: goToCreditsPage,
    visibility: ref(true),
  },
  { order: 5, label: '退出游戏', action: quitGame, visibility: ref(true) },
]
const router = useRouter()

onMounted(() =>
  setInterval(() => {
    loading.value = false
    progress.value = 100
  }, 2500),
)
function continueGame() {}
function newGame() {
  goToMainPage()
}
function openSave() {
  currentPage.value = 'save'
}
function openSettings() {
  currentPage.value = 'settings'
}
function quitGame() {
  window.close()
}
function goToMainPage() {
  console.log('准备跳转到主页面...')
  router.push('/')
}
function goToCreditsPage() {
  console.log('准备跳转到致谢页面...')
  router.push('/credit')
}
</script>

<style>
.body {
  width: 100%;
  height: 100%;
  position: relative;
  /* 必须保留，为 ::before 提供定位锚点 */
}

/* 定义自定义字体 */
@font-face {
  font-family: 'Maoken Assorted Sans';
  src: url('./assets/fonts/MaokenAssortedSans.ttf') format('truetype');
  font-weight: normal;
  font-style: normal;
  font-display: swap;
}

.body::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;

  /* 1. 背景图片现在只在这里定义 (使用修正后的路径) */
  background-image: url('../../assets/images/background.png');
  background-size: cover;
  background-position: center;

  /* 2. 修复：使用 z-index: 0 而不是 -1，确保背景可见 */
  z-index: 0;

  /* 3. 定义我们要过渡的属性：filter */
  /* 我们使用 filter: brightness() 来模拟蒙版，因为它可以平滑过渡 */
  filter: blur(0px) brightness(1);

  /* 使用 0.6s 的过渡时间 */
  transition: filter 0.6s cubic-bezier(0.7, 0, 0.2, 1);

  /* 确保不阻挡交互 */
  pointer-events: none;
}

.body.panel-active::before {
  filter: blur(12px) brightness(0.9);
  /* 通用背景模糊和变暗 */
}

/* 主容器，用于设置背景和布局 */
.menu-container {
  width: 100%;
  height: 100%;
  display: flex;
  justify-content: flex-start;
  /* 将主菜单推到左边 */
  align-items: center;
  position: relative;
  /* 必须保留，为 ::before 提供定位锚点 */
}

/* 为动画元素添加过渡效果 */
.logo,
.main-menu,
.settings-panel {
  transition:
    transform 0.6s cubic-bezier(0.7, 0, 0.2, 1),
    opacity 0.6s cubic-bezier(0.7, 0, 0.2, 1);
}

/* 主菜单 */
.main-menu {
  display: flex;
  flex-direction: column;
  /* align-items: center; */
  padding: 20px;
  margin-left: 10vw;
  /* 距离左侧边缘10%视口宽度 */
  position: relative;
  /* 确保菜单显示在背景之上 */
  z-index: 1;
}

.logo {
  position: absolute;
  top: 5vh;
  /* 距离顶部5%视口高度 */
  left: auto;
  right: 5vw;
  /* 距离右侧5%视口宽度 */
  height: 40vh;
  /* 高度为视口高度的40% */
  width: auto;
  /* 宽度自动，保持比例 */
  max-width: 40vw;
  /* 最大宽度不超过视口宽度的40%，防止过宽 */
  filter: drop-shadow(0 4px 8px rgba(0, 0, 0, 0.3));
  /* 添加一点阴影使其更突出 */
  z-index: 1;
  /* 确保logo显示在背景之上 */
}

.menu-item {
  background: transparent;
  /* 去除背景 */
  color: white;
  border: none;
  /* 去除边框 */
  padding: 15px;
  margin: 10px 0;
  border-radius: 12px;
  /* 使用clamp()实现响应式字体大小 */
  /* 最小32px, 根据视口宽度的4%缩放, 最大72px */
  font-size: clamp(32px, 4vw, 72px);
  font-weight: normal;
  /* 字体加粗 */
  font-family:
    'Maoken Assorted Sans',
    -apple-system,
    BlinkMacSystemFont,
    'Segoe UI',
    Roboto,
    'Helvetica Neue',
    Arial,
    sans-serif;
  /* 应用自定义字体，并提供备用字体 */
  cursor: pointer;
  transition:
    color 0.3s,
    text-shadow 0.3s;
  /* 平滑过渡 */
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
  /* 加一点文字阴影以保证清晰度 */
  text-align: justify;
  text-align-last: justify;
  /* 文字两端对齐 */
}

.menu-item:hover {
  color: #f0f0f0;
  text-shadow: 0 0 10px rgba(255, 255, 255, 0.8);
  /* 悬停时发光效果 */
  transform: none;
  /* 移除之前的缩放效果 */
}

.main-menu-animation-enter-active .main-menu,
.main-menu-animation-leave-active .main-menu,
.main-menu-animation-enter-active .logo,
.main-menu-animation-leave-active .logo {
  transition: all 0.3s ease-in-out;
}

.main-menu-animation-enter-from .main-menu,
.main-menu-animation-leave-to .main-menu {
  transform: translateX(-120%);
  opacity: 0;
}

.main-menu-animation-enter-from .logo,
.main-menu-animation-leave-active .logo {
  transform: translateX(120%);
  opacity: 0;
}

/* 特定面板的显示规则 */
body.panel-active.show-settings .settings-panel {
  transform: translateX(0);
  opacity: 1;
}

body.panel-active.show-game-screen .game-screen-panel {
  transform: translateX(0);
  opacity: 1;
  pointer-events: auto;
}

body.panel-active.show-load-save .load-save-panel {
  transform: translateX(0);
  opacity: 1;
  pointer-events: auto;
}
</style>
