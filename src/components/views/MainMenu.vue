<template>
  <div class="relative h-full w-full overflow-hidden">
    <MainChat v-if="currentPage === 'gameMainView'" />
    <!-- 设置面板的挂载时机单独用 settingsMounted 控制：关闭时要等退场动画播完再卸载 -->
    <Settings v-if="settingsMounted" slide-from-right @closed="onSettingsClosed" />

    <!-- 背景层（最底层） -->
    <div
      class="absolute top-0 left-[-10%] z-[-2] h-full w-[120%] bg-[url('../../assets/images/background2.png')] bg-cover bg-center will-change-transform"
      ref="bgRef"
    ></div>

    <!-- 流星层（SVG动画）— 临时暂停不污染持久偏好 -->
    <MeteorAnimation :meteors-enabled="effectiveMeteorsEnabled" :meteor-fps="meteorFps" />

    <!-- 星星粒子层（位于背景和人物之间） -->
    <StarAnimation
      :stars-enabled="effectiveStarsEnabled"
      :stars-layer-ref="starsLayerRef"
      :stars-fps="starsFps"
    />

    <!-- 人物图层（位于星星之上，菜单之下） -->
    <img
      class="pointer-events-none absolute top-1/2 left-1/2 z-3 max-h-full max-w-full transform-[translate(-50%,-50%)] will-change-transform select-none"
      ref="charRef"
      src="../../assets/images/alona.png"
      :alt="$t('views.mainMenu.characterAlt')"
      draggable="false"
      @contextmenu.prevent
    />

    <!-- 设置/存档页期间的背景压暗层。z-4 位于人物层(z-3)与菜单列(z-5)之间，
         只压暗背景与人物，退场中的菜单保持清晰。
         跟随 currentPage 而不是 settingsMounted：后者要等面板退场动画播完才置否，
         会让虚化在面板划走之后还残留一段 -->
    <Transition name="menu-dim">
      <div
        v-if="currentPage !== 'mainMenu'"
        class="pointer-events-none absolute inset-0 z-4 backdrop-blur-[12px] backdrop-brightness-90"
      ></div>
    </Transition>

    <!-- 菜单容器，绑定鼠标移动和移出事件实现视差 -->
    <Transition name="menu-page">
      <StartPage
        v-if="currentPage === 'mainMenu'"
        ref="containerRef"
        class="select-none"
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
          />
        </Transition>

        <!-- 游戏模式菜单 -->
        <Transition name="slide-right">
          <GameModeOptions
            v-if="menuState === 'gameMode'"
            @back="backToMainMenu"
            @open-scripts="showScriptModeMenu"
            @open-mini-games="showMiniGameMenu"
            :loadingScripts="loadingScripts"
            :scripts="scripts"
          />
        </Transition>

        <!-- 小游戏菜单沿用主菜单背景、字体和布局 -->
        <Transition name="slide-right">
          <MiniGameOptions v-if="menuState === 'miniGames'" @back="showGameModeMenu" />
        </Transition>

        <!-- 剧本模式菜单 -->
        <Transition name="slide-right">
          <ScriptModeOptions
            v-if="menuState === 'scriptMode'"
            @back="showGameModeMenu"
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

        <StartLogo @click="goToGithub" />
      </StartPage>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import type { WebInitData } from "@/api/services/game-info";
import { getScriptList, type ScriptSummary } from "@/api/services/script-info";
import { invoke } from "@tauri-apps/api/core";
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { useGameStore } from "../../stores/modules/game";
import { applyWebInitData } from "../../stores/modules/game/actions";
import { useSettingsStore } from "../../stores/modules/settings";
import { useUIStore } from "../../stores/modules/ui/ui";
import MeteorAnimation from "../game/standard/animations/MeteorAnimation.vue";
import { useParallaxAnimation } from "../game/standard/animations/ParallaxAnimation";
import StarAnimation from "../game/standard/animations/StarAnimation.vue";
import { SettingsPanel as Settings } from "../settings/";
import MainChat from "./MainChat.vue";
import { StartLogo, StartPage } from "./menu/base";
import {
  GameModeOptions,
  MiniGameOptions,
  MainMenuOptions,
  ScriptModeOptions,
  WorkshopOptions,
} from "./menu/page";

const props = withDefaults(defineProps<{ initialMenu?: "main" | "miniGames" }>(), {
  initialMenu: "main",
});
const { t } = useI18n();
const router = useRouter();
const uiStore = useUIStore();
const settingsStore = useSettingsStore();

// 页面与菜单状态
const currentPage = ref("mainMenu");
// 设置面板是否挂载。与 currentPage 分开是因为关闭时面板要留到退场动画播完才卸载，
// 而菜单需要立刻回场与它重叠
const settingsMounted = ref(false);
const menuState = ref<"main" | "gameMode" | "scriptMode" | "workshop" | "miniGames">(
  props.initialMenu,
);
const scripts = ref<ScriptSummary[]>([]);
const loadingScripts = ref(false);
const starsEnabled = computed(() => settingsStore.mainMenuStarsEnabled);
const meteorsEnabled = computed(() => settingsStore.mainMenuMeteorsEnabled);
const meteorFps = computed(() => settingsStore.meteorFps);
const starsFps = computed(() => settingsStore.starsFps);

const transientSuspend = ref(false);
const effectiveStarsEnabled = computed(() => starsEnabled.value && !transientSuspend.value);
const effectiveMeteorsEnabled = computed(() => meteorsEnabled.value && !transientSuspend.value);
const parallaxEnabled = computed(() => !transientSuspend.value);

// DOM Refs
const containerRef = ref<HTMLElement | null>(null);
const bgRef = ref<HTMLElement | null>(null);
const charRef = ref<HTMLElement | null>(null);
const starsLayerRef = ref<HTMLElement | null>(null);

/* ================== 菜单逻辑 ================== */
function showGameModeMenu() {
  menuState.value = "gameMode";
}
function showMiniGameMenu() {
  menuState.value = "miniGames";
}
function handleOpenCredits() {
  router.push("/credit");
}
function backToMainMenu() {
  menuState.value = "main";
}
function showScriptModeMenu() {
  menuState.value = "scriptMode";
}
function showWorkshopMenu() {
  menuState.value = "workshop";
}
function goToGithub() {
  window.open("https://github.com/SlimeBoyOwO/LingChat", "_blank");
}

const handleContinueGame = async () => {
  try {
    const { saves } = await invoke<{ saves: Array<{ id: number }>; total: number }>(
      "list_saves",
      {
        page: 1,
        pageSize: 1,
      },
    );
    if (!saves || saves.length === 0) {
      uiStore.showWarning({
        title: t("views.mainMenu.noSaveTitle"),
        message: t("views.mainMenu.noSaveMessage"),
      });
      return;
    }
    const gameInfo = await invoke<WebInitData>("load_save", { saveId: saves[0].id });
    const gameStore = useGameStore();
    applyWebInitData(gameStore.$state, gameInfo);
    router.push("/chat");
  } catch (error) {
    console.error("继续游戏失败:", error);
    uiStore.showError({
      title: t("views.mainMenu.continueFailTitle"),
      message: t("views.mainMenu.continueFailMessage"),
    });
  }
};

function handleOpenSettings(tab?: string) {
  // 与菜单列退场、背景压暗层同帧发生：菜单列由外层 Transition 向左滑出，
  // 设置面板自身从右滑入（slide-from-right），背景在 0.3s 内渐暗
  uiStore.toggleSettings(true);
  if (tab === "save") {
    currentPage.value = "save";
    uiStore.setSettingsTab("save");
  } else {
    currentPage.value = "settings";
  }
  settingsMounted.value = true;
  // 设置页常驻期间暂停星星与流星
  transientSuspend.value = true;
}

// 面板退场动画播完后才真正卸载，并恢复主菜单的粒子动画
function onSettingsClosed() {
  settingsMounted.value = false;
  if (transientSuspend.value) transientSuspend.value = false;
}

watch(
  () => uiStore.showSettings,
  (newVal) => {
    // 菜单立刻回场（左滑进入），与面板向右退场重叠；面板卸载见 onSettingsClosed
    if (!newVal && currentPage.value !== "mainMenu") {
      currentPage.value = "mainMenu";
      menuState.value = "main";
    }
  },
);

/* ================== 视差动画 Hook ================== */
const { handleMouseMove, handleMouseLeave } = useParallaxAnimation(
  {
    charRef,
    bgRef,
    starsLayerRef,
  },
  {},
  parallaxEnabled,
);

// 抽取接口请求逻辑，不阻塞动画初始化
async function fetchScripts() {
  loadingScripts.value = true;
  try {
    scripts.value = await getScriptList();
  } catch (e) {
    uiStore.showError({
      errorCode: "script_list_failed",
      message: t("views.mainMenu.scriptListFailed"),
    });
    scripts.value = [];
  } finally {
    loadingScripts.value = false;
  }
}

onMounted(() => {
  const initializeMenu = async () => {
    // 性能提示只显示一次
    const PERFORMANCE_TIP_KEY = "mainMenuPerformanceTipShown";
    if (
      (starsEnabled.value || meteorsEnabled.value) &&
      !localStorage.getItem(PERFORMANCE_TIP_KEY)
    ) {
      localStorage.setItem(PERFORMANCE_TIP_KEY, "true");
      uiStore.showInfo({
        title: "Tip",
        message: t("views.mainMenu.perfTip"),
        duration: 5000,
      });
    }

    fetchScripts();
  };

  initializeMenu();
});
</script>

<style scoped>
@font-face {
  font-family: "Maoken Assorted Sans";
  src: url("/fonts/MaokenAssortedSans.woff2") format("woff2");
  font-weight: normal;
  font-style: normal;
  font-display: swap;
}

/* 菜单容器 */

/* 页面切换动画 */
.slide-left-enter-active,
.slide-left-leave-active,
.slide-right-enter-active,
.slide-right-leave-active {
  transition: all 0.4s cubic-bezier(0.7, 0, 0.2, 1);
}

/* Remove leaving elements from flex flow immediately to prevent layout jump；
   退场期间不再接收点击 */
.slide-left-leave-active,
.slide-right-leave-active {
  position: absolute;
  pointer-events: none;
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

/* 菜单列整体退场：观感与开始游戏进二级菜单一致地左滑淡出。
   只定义 leave 不定义 enter —— 回场时若整列也位移，会与内部菜单自身的
   slide-left 叠加成两倍行程；即时出现，回场动感交给内部菜单与压暗层渐隐 */
.menu-page-leave-active {
  transition: all 0.4s cubic-bezier(0.7, 0, 0.2, 1);
  position: absolute;
  pointer-events: none;
}

.menu-page-leave-to {
  transform: translateX(-120%);
  opacity: 0;
}

/* 设置页期间的背景压暗层。进场 0.3s 与设置面板自带遮罩同步；
   离场 0.42s 与面板向右滑出的时长一致，两者同时结束，不留残余 */
.menu-dim-enter-active {
  transition: opacity 0.3s ease;
}

.menu-dim-leave-active {
  transition: opacity 0.42s ease;
}

.menu-dim-enter-from,
.menu-dim-leave-to {
  opacity: 0;
}
</style>
