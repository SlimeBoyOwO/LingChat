<template>
  <!-- 根元素上的四个 .xx-safe 工具类（定义在 src/assets/styles/base.css）：
       手机端设置页是**独立 Activity**，同样跑 edge-to-edge（targetSdk 36，
       Android 15+ 强制），不内缩就会被状态栏 / 手势条压住。
       变量 --safe-area-inset-* 由 SettingsActivity 通过 SafeAreaInsets 注入
       （Android WebView 的 env(safe-area-inset-*) 恒为 0，不可用）；
       桌面端这些变量恒为 0px，因此是零回归的空操作。

       max-md:rounded-none：窄屏下设置页铺满整屏，圆角会切出四个小三角露出
       窗口底色，干脆在 <768px 去掉圆角。 -->
  <div
    :class="[
      `pt-safe pr-safe pb-safe pl-safe relative flex h-full w-full flex-col overflow-hidden rounded-xl border font-sans transition-colors duration-300 max-md:rounded-none`,
      isDarkMode
        ? 'dark border-slate-700 bg-slate-900 text-slate-200 selection:bg-sky-800'
        : 'border-slate-200 bg-[#FAFCFF] text-slate-800 selection:bg-sky-200',
    ]"
  >
    <div
      class="bg-grid-pattern pointer-events-none absolute inset-0 z-0 opacity-40 transition-colors duration-300"
      style="background-size: 24px 24px"
    ></div>

    <section
      class="relative z-10 m-1 flex h-full w-full flex-col overflow-hidden rounded-xl border shadow-2xl backdrop-blur-md transition-colors duration-300 md:m-2"
      :class="isDarkMode ? 'border-slate-700 bg-slate-800/70' : 'border-slate-200 bg-white/60'"
      style="height: calc(100% - 1rem); width: calc(100% - 1rem)"
    >
      <SettingsHeader
        :isDarkMode="isDarkMode"
        :isMaximized="isMaximized"
        @toggleTheme="toggleTheme"
        @minimizeWindow="minimizeWindow"
        @toggleMaximizeWindow="toggleMaximizeWindow"
        @closeWindow="closeWindow"
      />

      <main class="relative flex flex-1 overflow-hidden">
        <SettingsSidebar
          :isDarkMode="isDarkMode"
          :activeTab="activeTab"
          :tabs="tabs"
          @update:activeTab="activeTab = $event"
        />

        <section class="relative z-10 flex-1 overflow-y-auto scroll-smooth p-6 md:p-8">
          <transition name="fade-slide" mode="out-in">
            <PetTab
              v-if="activeTab === 'pet'"
              key="pet"
              :isDarkMode="isDarkMode"
              :petScale="petScale"
              :PET_SCALE_MIN="PET_SCALE_MIN"
              :PET_SCALE_MAX="PET_SCALE_MAX"
              :petVolume="petVolume"
              :live2dFps="petLive2dFps"
              :petBubbleSide="petBubbleSide"
              @updateScale="updateScale"
              @resetScale="resetScale"
              @updateVolume="updateVolume"
              @resetVolume="resetVolume"
              @updateLive2dFps="updateLive2dFps"
              @resetLive2dFps="resetLive2dFps"
              @updateBubbleSide="updateBubbleSide"
            />
            <HistoryTab
              v-else-if="activeTab === 'interaction'"
              key="interaction"
              :isDarkMode="isDarkMode"
            />
            <WindowTab v-else-if="activeTab == 'window'" key="window" :isDarkMode="isDarkMode" />
            <TodoTab v-else key="todo" :isDarkMode="isDarkMode" />
          </transition>
        </section>
      </main>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
// 替换为你项目中实际存在的 store 路径
import {
  DEFAULT_SETTINGS,
  useSettingsStore,
  type BubbleSide,
} from "../../../stores/modules/settings";

const PET_SCALE_DEFAULT = 1.0;
const PET_SCALE_MAX = 1.3;
const PET_SCALE_MIN = 0.7;
import { useGameStore } from "../../../stores/modules/game";
import { isMobile } from "@/utils/platform";

// 引入 Lucide 图标
import { Ruler, Book, Cat, CheckCircle2 } from "lucide-vue-next";

// 引入自定义组件
import SettingsHeader from "../components/SettingsHeader.vue";
import SettingsSidebar from "../components/SettingsSidebar.vue";
import { PetTab, HistoryTab, WindowTab } from "../components/tabs";
import TodoTab from "../components/tabs/TodoTab.vue";
const PET_SCALE_EVENT = "pet-scale-changed";
const PET_VOLUME_EVENT = "pet-volume-changed";
const PET_LIVE2D_FPS_EVENT = "pet-live2d-fps-changed";
const PET_BUBBLE_SIDE_EVENT = "pet-bubble-side-changed";
const DIALOG_HISTORY_EVENT = "dialog-history-changed";
const DARK_MODE_KEY = "lingchat-dark-mode";
const appWindow = getCurrentWindow();
const settingsStore = useSettingsStore();
const gameStore = useGameStore();
const { t } = useI18n();

const isMaximized = ref(false);
const activeTab = ref<"pet" | "interaction" | "window" | "todo">("pet");

// 深色模式状态与切换方法
const isDarkMode = ref(false);

// 从 localStorage 加载深色模式设置
const loadDarkModeFromStorage = () => {
  const savedDarkMode = localStorage.getItem(DARK_MODE_KEY);
  if (savedDarkMode !== null) {
    isDarkMode.value = savedDarkMode === "true";
  }
};

// 将深色模式设置保存到 localStorage
const saveDarkModeToStorage = () => {
  localStorage.setItem(DARK_MODE_KEY, String(isDarkMode.value));
};

const toggleTheme = () => {
  isDarkMode.value = !isDarkMode.value;
  saveDarkModeToStorage();
};

type TabItem = {
  key: "pet" | "interaction" | "window" | "todo";
  label: string;
  icon: any;
  en: string;
};

const tabs = computed(() => [
  { key: "pet", label: t("pet.tabs.pet"), icon: Ruler, en: "PET CONFIG" } as TabItem,
  {
    key: "interaction",
    label: t("pet.tabs.interaction"),
    icon: Book,
    en: "HISTORY DIALOGUE",
  } as TabItem,
  {
    key: "todo",
    label: t("pet.tabs.todo"),
    icon: CheckCircle2,
    en: "TODO LIST",
  } as TabItem,
  {
    key: "window",
    label: t("pet.tabs.window"),
    icon: Cat,
    en: "PROACTIVE SYSTEM",
  } as TabItem,
]);

const petScale = computed(() => settingsStore.pet.scale);
const petVolume = computed(() => settingsStore.characterVolume);
const petLive2dFps = computed(() => settingsStore.pet.live2dFps ?? 30);
const petBubbleSide = computed(() => settingsStore.pet.bubbleSide);

const syncMaximizedState = async () => {
  // 移动端没有「最大化」概念，`isMaximized()` 在 Android 上行为不确定。
  // 这里必须兜住：`onMounted` 里是 `await syncMaximizedState()`，
  // 一旦抛异常，后面的深色模式加载与事件监听就都不会执行了。
  try {
    isMaximized.value = await appWindow.isMaximized();
  } catch {
    isMaximized.value = false;
  }
};

const emitScaleChanged = async (scale: number) => {
  await appWindow.emit(PET_SCALE_EVENT, { scale });
};

const updateScale = async (scale: number) => {
  settingsStore.pet.scale = scale;
  await emitScaleChanged(settingsStore.pet.scale);
};

const resetScale = async () => {
  await updateScale(PET_SCALE_DEFAULT);
};

const emitVolumeChanged = async (volume: number) => {
  await appWindow.emit(PET_VOLUME_EVENT, { volume });
};

const updateVolume = async (volume: number) => {
  const normalizedVolume = Math.min(100, Math.max(0, Math.round(volume)));
  settingsStore.updateAudio({ characterVolume: normalizedVolume });
  await emitVolumeChanged(normalizedVolume);
};

const resetVolume = async () => {
  await updateVolume(DEFAULT_SETTINGS.audio.characterVolume);
};

const emitLive2dFpsChanged = async (fps: number) => {
  await appWindow.emit(PET_LIVE2D_FPS_EVENT, { fps });
};

const updateLive2dFps = async (fps: number) => {
  settingsStore.setPetLive2dFps(fps);
  await emitLive2dFpsChanged(fps);
};

const resetLive2dFps = async () => {
  await updateLive2dFps(DEFAULT_SETTINGS.pet.live2dFps);
};

const updateBubbleSide = async (side: BubbleSide) => {
  settingsStore.pet.bubbleSide = side;
  await appWindow.emit(PET_BUBBLE_SIDE_EVENT, { side });
};

const minimizeWindow = async () => {
  // 桌面端专属：手机上头部已经不显示这个按钮（见 SettingsHeader）。
  await appWindow.minimize();
};

const toggleMaximizeWindow = async () => {
  await appWindow.toggleMaximize();
  await syncMaximizedState();
};

/**
 * 关闭设置窗口。
 *
 * ## 手机端必须走原生 `finish()`
 *
 * 桌面端 `Window.close()` 是关掉一个系统窗口，Android 上**没有对应实现** ——
 * 翻 `tao` 的 `platform_impl/android/mod.rs`，`Window` 上没有 `close()` /
 * `destroy()`，`on_window_close` 只是把 Rust 侧的 wrapper 置空。
 * 直接调的结果是：WebView 被销毁、承载它的 Activity 还在 → **一块黑屏**。
 *
 * 所以手机端改调 `SettingsActivity` 通过 `onWebViewCreate` 注入的接口
 * （见 `gen/android/.../com/noiq/lingchat/SettingsActivity.kt`），由原生
 * `finish()` 结束那个 Activity。
 *
 * 接口拿不到时（理论上不该发生）退回原来的行为，至少不会更差。
 */
const closeWindow = async () => {
  if (isMobile()) {
    const bridge = (window as unknown as { LingChatSettings?: { close?: () => void } })
      .LingChatSettings;
    if (typeof bridge?.close === "function") {
      bridge.close();
      return;
    }
  }
  await appWindow.close();
};

onMounted(async () => {
  await syncMaximizedState();

  // 从 localStorage 加载深色模式设置
  loadDarkModeFromStorage();

  // 先注册监听，再请求数据（避免响应在监听就绪前到达而被丢弃）
  const unlisten = await appWindow.listen<{ dialogHistory: any[] }>(
    DIALOG_HISTORY_EVENT,
    (event) => {
      const { dialogHistory } = event.payload;
      if (dialogHistory) {
        gameStore.dialogHistory = dialogHistory;
      }
    },
  );

  // 向主窗口请求当前历史数据（主窗口会响应 dialog-history-changed）
  await appWindow.emit("request-dialog-history");

  // 组件卸载时取消监听
  onUnmounted(() => {
    unlisten();
  });
});
</script>

<style scoped>
/* 浅色模式网格 */
.bg-grid-pattern {
  background-image: radial-gradient(circle, #cbd5e1 1px, transparent 1px);
}

/* 深色模式网格 */
.dark .bg-grid-pattern {
  background-image: radial-gradient(circle, #334155 1px, transparent 1px);
}

/* 页面切换动画 */
.fade-slide-enter-active,
.fade-slide-leave-active {
  transition:
    opacity 0.25s ease,
    transform 0.25s cubic-bezier(0.2, 0.8, 0.2, 1);
}

.fade-slide-enter-from {
  opacity: 0;
  transform: translateY(10px);
}

.fade-slide-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}
</style>
