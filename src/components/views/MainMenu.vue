<template>
  <div class="w-full h-full relative overflow-hidden" :class="panelClass">
    <MainChat v-if="currentPage === 'gameMainView'" />
    <Settings v-else-if="currentPage === 'settings'" />
    <Save v-else-if="currentPage === 'save'" />

    <!-- 背景层（最底层） -->
    <div
      class="absolute top-0 left-[-10%] w-[120%] h-full bg-cover bg-center bg-[url('../../assets/images/background2.png')] z-[-2] will-change-transform"
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
      class="absolute top-1/2 left-1/2 transform-[translate(-50%,-50%)] max-w-full max-h-full z-[3] pointer-events-none will-change-transform"
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
  </div>
</template>

<script setup lang="ts">
  import type { WebInitData } from "@/api/services/game-info";
  import { getScriptList, type ScriptSummary } from "@/api/services/script-info";
  import { useHideForSnapshot } from "@/composables/useHideForSnapshot";
  import { useSettingsSnapshot } from "@/composables/useSettingsSnapshot";
  import { isWindows } from "@/utils/platform";
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
    MainMenuOptions,
    ScriptModeOptions,
    WorkshopOptions,
  } from "./menu/page";

  const { t } = useI18n();
  const router = useRouter();
  const uiStore = useUIStore();
  const settingsStore = useSettingsStore();

  // 页面与菜单状态
  const currentPage = ref("mainMenu");
  const menuState = ref<"main" | "gameMode" | "scriptMode" | "workshop">("main");
  const scripts = ref<ScriptSummary[]>([]);
  const loadingScripts = ref(false);
  const starsEnabled = computed(() => settingsStore.mainMenuStarsEnabled);
  const meteorsEnabled = computed(() => settingsStore.mainMenuMeteorsEnabled);
  const meteorFps = computed(() => settingsStore.meteorFps);
  const starsFps = computed(() => settingsStore.starsFps);

  // 临时暂停（内存态，不写入持久化）— 仅 Windows 快照期间生效
  const isWindowsMode = computed(() => isWindows());
  const transientSuspend = ref(false);
  const effectiveStarsEnabled = computed(() => starsEnabled.value && !transientSuspend.value);
  const effectiveMeteorsEnabled = computed(() => meteorsEnabled.value && !transientSuspend.value);
  const parallaxEnabled = computed(() => !transientSuspend.value);
  const panelClass = computed(() => {
    if (currentPage.value === "mainMenu") return "";
    // Windows 快照态：不做实时模糊，静态快照已在 SettingsPanel 内
    if (isWindowsMode.value) return "";
    return "before:content-[''] before:absolute before:inset-0 before:backdrop-blur-[12px] before:backdrop-brightness-90 before:z-10 before:pointer-events-none";
  });
  const settingsSnapshot = useSettingsSnapshot();
  const { hide: hideForSnapshot, restore: restoreForSnapshot, resolveEl } = useHideForSnapshot();
  let settingsSnapshotSession: number | null = null;

  // DOM Refs
  const containerRef = ref<HTMLElement | null>(null);
  const bgRef = ref<HTMLElement | null>(null);
  const charRef = ref<HTMLElement | null>(null);
  const starsLayerRef = ref<HTMLElement | null>(null);

  const Save = Settings;

  /* ================== 菜单逻辑 ================== */
  function showGameModeMenu() {
    menuState.value = "gameMode";
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
        }
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

  async function handleOpenSettings(tab?: string) {
    // Windows：非阻塞快照 — hide → capture → 立即开设置 → await → finally restore
    // 设置页下一帧即以 dim 占位出现，快照后台 0~800ms 就绪后淡入替换，不阻塞打开
    if (isWindowsMode.value) {
      const el = resolveEl(containerRef.value);
      // 后台执行隐藏与捕获，不阻塞设置页打开
      (async () => {
        let capturePromise: Promise<string | null> | null = null;
        try {
          await hideForSnapshot(el);
          capturePromise = settingsSnapshot.capture();
          // 立即打开设置（按钮仍 hidden，不会被拍）
          uiStore.toggleSettings(true);
          if (tab === "save") {
            currentPage.value = "save";
            uiStore.setSettingsTab("save");
          } else {
            currentPage.value = "settings";
          }
          const result = await capturePromise;
          if (result) {
            settingsSnapshotSession = settingsSnapshot.snapshotSessionId.value || null;
          }
        } catch (e) {
          console.warn("[MainMenu] snapshot capture error:", e);
        } finally {
          restoreForSnapshot(el);
          // 截图完成后才暂停动画，需守卫：若用户已快速关闭设置则不再暂停
          if (uiStore.showSettings && currentPage.value !== "mainMenu") {
            transientSuspend.value = true;
          }
        }
        if (capturePromise) {
          capturePromise.catch(() => restoreForSnapshot(el));
        }
      })();
      return;
    }
    uiStore.toggleSettings(true);
    if (tab === "save") {
      currentPage.value = "save";
      uiStore.setSettingsTab("save");
    } else {
      currentPage.value = "settings";
    }
  }

  watch(
    () => uiStore.showSettings,
    (newVal) => {
      if (!newVal && (currentPage.value === "settings" || currentPage.value === "save")) {
        currentPage.value = "mainMenu";
        menuState.value = "main";
        // 恢复动画（按最新持久值）
        if (transientSuspend.value) transientSuspend.value = false;
        // 释放静态背景临时资源（session守卫）
        if (isWindowsMode.value && settingsSnapshotSession !== null) {
          const sid = settingsSnapshotSession;
          settingsSnapshotSession = null;
          settingsSnapshot.release(sid).catch(() => {});
        } else if (isWindowsMode.value) {
          settingsSnapshot.release().catch(() => {});
        }
      }
    }
  );

  /* ================== 视差动画 Hook ================== */
  const { handleMouseMove, handleMouseLeave } = useParallaxAnimation(
    {
      charRef,
      bgRef,
      starsLayerRef,
    },
    {},
    parallaxEnabled
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
</style>
