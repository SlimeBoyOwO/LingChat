<template>
  <!-- 根元素上的四个 .xx-safe 工具类（定义在 src/assets/styles/base.css）：
       手机端设置页是**独立 Activity**，同样跑 edge-to-edge（targetSdk 36，
       Android 15+ 强制），不内缩就会被状态栏 / 手势条压住。
       变量 --safe-area-inset-* 由 SettingsActivity 通过 SafeAreaInsets 注入
       （Android WebView 的 env(safe-area-inset-*) 恒为 0，不可用）；
       桌面端这些变量恒为 0px，因此是零回归的空操作。

       max-[768px]:rounded-none：窄屏下设置页铺满整屏，圆角会切出四个小三角露出
       窗口底色，干脆在 <768px 去掉圆角。 -->
  <div
    :class="[
      `pt-safe pr-safe pb-safe pl-safe relative flex h-full w-full flex-col overflow-hidden rounded-xl border font-sans transition-colors duration-300 max-[768px]:rounded-none`,
      isDarkMode
        ? 'dark border-slate-700 bg-slate-900 text-slate-200 selection:bg-sky-800'
        : 'border-slate-200 bg-[#FAFCFF] text-slate-800 selection:bg-sky-200',
    ]"
  >
    <div
      class="bg-grid-pattern pointer-events-none absolute inset-0 z-0 opacity-40 transition-colors duration-300"
      style="background-size: 24px 24px"
    ></div>

    <!-- ★ 内层卡片：靠 flex 拉伸取尺寸，**不要**再写 `w-full` / `h-full` 或
         `calc(100% - 1rem)`（2026-10-03 三修）。

         之前是 `h-full w-full` + 行内 `width: calc(100% - 1rem)` 配 `m-1`，
         三者互相打架，结果是**左右不对称**：
           · 行内 width 覆盖掉 w-full → 卡片宽 = 父内容宽 − 1rem(16px)
           · 左边距只有 m-1 的 4px
           · 于是右边空出 4 + (16 − 4) = 12px，左边只有 4px
         真机表现（用户原话）：「设置页右边多一小段空白」。用户猜是 Vue 问题，
         猜对了 —— 跟安全区、跟原生都无关，纯粹是这里的尺寸算式。

         现在只留 `m-1`（窄屏 4px / `min-[768px]:m-2` 8px），宽度交给父容器的
         `align-items: stretch`（`flex-col` 默认值），高度交给 `flex-1`，
         四边边距天然对称。`min-h-0` 是让内部的 `overflow-hidden` 生效。 -->
    <section
      class="relative z-10 m-1 flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border shadow-2xl backdrop-blur-md transition-colors duration-300 min-[768px]:m-2"
      :class="isDarkMode ? 'border-slate-700 bg-slate-800/70' : 'border-slate-200 bg-white/60'"
    >
      <SettingsHeader
        :isDarkMode="isDarkMode"
        :isMaximized="isMaximized"
        @toggleTheme="toggleTheme"
        @minimizeWindow="minimizeWindow"
        @toggleMaximizeWindow="toggleMaximizeWindow"
        @closeWindow="closeWindow"
      />

      <!-- ★ 方向必须是**窄屏竖排、宽屏横排** —— 这是整个响应式的枢纽。
           这里曾经漏了 `flex-col`：`SettingsSidebar` 在窄屏是
           `w-full shrink-0`（顶部横向标签条），而 `main` 还是 `flex`（横向）——
           于是标签条把整行宽度吃光，内容区被压成 **0 宽** 并被 `overflow-hidden`
           裁掉。真机表现：竖屏进设置页只看到一条标签条，下面一片空白，
           点标签也没反应（内容确实切换了，只是宽度是 0 看不见）。
           `min-[768px]:` 以上恢复成原来的「左侧竖栏 + 右侧内容」。

           用 `min-[768px]:` 而**不是** `md:`：`md:` 是 48rem，而根字号被
           `clamp(12px, min(2.5vw,2.5vh), 16px)` 缩放过了（手机 12px → 576px，
           桌面 16px → 768px），断点会漂移。见 SettingsSidebar 头部的说明。 -->
      <main class="relative flex flex-1 flex-col overflow-hidden min-[768px]:flex-row">
        <SettingsSidebar
          :isDarkMode="isDarkMode"
          :activeTab="activeTab"
          :tabs="tabs"
          @update:activeTab="activeTab = $event"
        />

        <!-- 左右滑动切标签：与 LC 自己的设置面板（components/settings/SettingsPanel.vue）
             同一套实现 —— 那里已经跑过一段时间，判断条件（排除滑块 / 横向滚动容器、
             只认明显水平滑动）都是踩过坑才定下来的，这里直接照搬，不另起一套。

             挂在**内容区**而不是整页：标签条本身是横向可滚动的（窄屏下四个标签
             排在顶部横条里），挂在整页会把「滑标签条」也吃成切页。 -->
        <section
          ref="contentRef"
          class="relative z-10 flex-1 overflow-x-hidden overflow-y-auto scroll-smooth p-6 min-[768px]:p-8"
          @touchstart="onTouchStart"
          @touchend="onTouchEnd"
        >
          <transition :name="transitionName" mode="out-in">
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
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
// 替换为你项目中实际存在的 store 路径
import {
  DEFAULT_SETTINGS,
  useSettingsStore,
  type BubbleSide,
} from "../../../stores/modules/settings";
import { useUIStore } from "../../../stores/modules/ui/ui";

const PET_SCALE_DEFAULT = 1.0;
const PET_SCALE_MAX = 1.3;
const PET_SCALE_MIN = 0.7;
import { useGameStore } from "../../../stores/modules/game";
import { isMobile } from "@/utils/platform";

// 引入 Lucide 图标
import { Ruler, Book, Cat, CheckCircle2 } from "@lucide/vue";

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
const uiStore = useUIStore();
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

// ========== 手机端左右滑动切换标签 ==========
//
// 与 LC 自己的设置面板（`components/settings/SettingsPanel.vue`）**同一套实现**：
// 那边已经跑过一段时间，几个判断条件都是踩过坑才定下来的，这里照搬，不另起一套。
//
// 顺序必须与 SettingsSidebar 收到的 `tabs` 一致 —— 滑动方向靠索引比较得出。
const SWIPE_TABS = ["pet", "interaction", "todo", "window"] as const;

// 分页转场方向：左滑（下一项）→ slide-left（新页从右侧进）；右滑（上一项）→ slide-right。
// watch 默认 flush: 'pre'，在重渲染前更新 transitionName，Transition 开始动画时能取到正确方向。
const transitionName = ref<"slide-left" | "slide-right">("slide-left");

watch(activeTab, (newTab, oldTab) => {
  if (!oldTab || oldTab === newTab) return;
  const newIdx = SWIPE_TABS.indexOf(newTab);
  const oldIdx = SWIPE_TABS.indexOf(oldTab);
  // 目标 / 来源不在滑动顺序里（理论不应发生）→ 保持原方向
  if (newIdx === -1 || oldIdx === -1) return;
  transitionName.value = newIdx > oldIdx ? "slide-left" : "slide-right";
});

const contentRef = ref<HTMLElement | null>(null);
let touchStartX = 0;
let touchStartY = 0;
let touchOnHorizontalScrollable = false;
let isSwipeAnimating = false;

/**
 * 触摸起点是否落在「可滚动」容器内（数值滑块、横向滚动表格等）。
 *
 * 是则不切页 —— 用户多半是在拖滑块或滚横向内容，不是在翻页。
 * 只排除**确实有溢出可滚**的容器：内容不满的页面（无可滚动区域）仍可滑动切换。
 * 竖向滚动容器不在此列：竖向滚动由 [onTouchEnd] 里 `|dx| <= |dy|` 兜底。
 */
function isInsideScrollable(el: Element | null): boolean {
  while (el && el !== contentRef.value) {
    // 数值调节滑块（原生 range / 自定义 Slider）→ 拖动它不该切页
    if (el.tagName === "INPUT" && (el as HTMLInputElement).type === "range") return true;
    const s = getComputedStyle(el);
    if (
      (s.overflowX === "auto" || s.overflowX === "scroll") &&
      el.scrollWidth > el.clientWidth + 4
    ) {
      return true;
    }
    el = el.parentElement;
  }
  return false;
}

/**
 * 是否允许滑动切页。
 *
 * 门槛用 **768px 可用宽度**，与 `SettingsSidebar` 的断点一致：窄屏时四个标签是
 * 顶部横条，左右滑动顺手；宽屏是左侧竖栏，滑动反而容易误触。
 *
 * ⚠️ 不用 `uiStore.isSmallScreen`（那是 `min(宽,高) < 500`）：手机横屏约 780×360
 * 时它判为「小屏」，而布局早就切成宽屏竖栏了，两者会打架。
 */
const canSwipe = computed(() => uiStore.viewportWidth < 768);

const onTouchStart = (e: TouchEvent) => {
  touchStartX = e.touches[0].clientX;
  touchStartY = e.touches[0].clientY;
  touchOnHorizontalScrollable = isInsideScrollable(e.target as Element);
};

const onTouchEnd = (e: TouchEvent) => {
  if (!canSwipe.value) return;
  // 起点在可横向滚动区域（历史表格等）→ 让原生滚动处理
  if (touchOnHorizontalScrollable) return;
  if (isSwipeAnimating) return;

  const dx = e.changedTouches[0].clientX - touchStartX;
  const dy = e.changedTouches[0].clientY - touchStartY;

  // 只响应明显的水平滑动（避免和垂直滚动 / 滑块冲突）
  if (Math.abs(dx) < 50 || Math.abs(dx) <= Math.abs(dy)) return;

  const currentIdx = SWIPE_TABS.indexOf(activeTab.value);
  if (currentIdx === -1) return;
  let nextIdx = dx < 0 ? currentIdx + 1 : currentIdx - 1; // 左滑 → 下一个，右滑 → 上一个
  if (nextIdx < 0) nextIdx = SWIPE_TABS.length - 1;
  if (nextIdx >= SWIPE_TABS.length) nextIdx = 0;

  // 动画期间屏蔽连击，否则快速连划会一次跳好几页
  isSwipeAnimating = true;
  activeTab.value = SWIPE_TABS[nextIdx];
  setTimeout(() => {
    isSwipeAnimating = false;
  }, 300);
};

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
 * ## 手机端必须走原生桥，而且**不是** `finish()`
 *
 * 桌面端 `Window.close()` 是关掉一个系统窗口，Android 上**没有对应实现** ——
 * 翻 `tao` 的 `platform_impl/android/mod.rs`，`Window` 上没有 `close()` /
 * `destroy()`，`on_window_close` 只是把 Rust 侧的 wrapper 置空。
 * 直接调的结果是：WebView 被销毁、承载它的 Activity 还在 → **一块黑屏**。
 *
 * 所以手机端改调 `SettingsActivity` 通过 `onWebViewCreate` 注入的接口
 * （见 `gen/android/.../com/noiq/lingchat/SettingsActivity.kt`）。
 *
 * ⚠️ **2026-10-03 起那个接口做的是 `moveTaskToBack`，不再是 `finish()`**：
 *
 * - `finish()` 会销毁设置窗（每次重开都要重建 WebView + 重载页面），
 *   而且**会落到 MainActivity，也就是聊天页** —— 用户报的「关掉设置却回到
 *   聊天页」就是这条；
 * - `moveTaskToBack` 只把整个任务退到后台，设置窗（连同标签页选择、
 *   滚动位置）原样保留，再点「设置」由原生拉回前台即可。
 *
 * 语义的完整说明与「推到后台就隐藏」的守卫见 `SettingsActivity` 头注释。
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

/* 分页转场：与 LC 设置面板（SettingsPanel.vue）同一套参数。
   用「起步即快」的缓动（而非默认的慢起步）—— 切 Tab 时新页挂载 / 旧页卸载
   会有 1~2 帧主线程开销，慢起步缓动会让旧页看起来「卡在原地」形成残留。 */
.slide-left-enter-active,
.slide-left-leave-active,
.slide-right-enter-active,
.slide-right-leave-active {
  transition: transform 0.32s cubic-bezier(0.32, 0.72, 0, 1);
}

.slide-left-enter-from {
  transform: translateX(100%);
}
.slide-left-leave-to {
  transform: translateX(-100%);
}

.slide-right-enter-from {
  transform: translateX(-100%);
}
.slide-right-leave-to {
  transform: translateX(100%);
}

.slide-left-enter-to,
.slide-left-leave-from,
.slide-right-enter-to,
.slide-right-leave-from {
  transform: translateX(0);
}

/* ⚠️ 位移会被内容区裁掉，所以 `section` 上必须有 `overflow-x-hidden`
   （LC 那边是靠外层 `overflow-hidden` + 子元素 `absolute inset-0` 做到的）。
   不加的话横向溢出会把内容区变成可横向滚动的容器，滑动切页之后能横向拖走一截。 */
</style>
