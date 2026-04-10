<template>
  <div
    :class="[
      'relative w-full h-full rounded-xl border flex flex-col overflow-hidden font-sans transition-colors duration-300',
      isDarkMode
        ? 'dark bg-slate-900 border-slate-700 text-slate-200 selection:bg-sky-800'
        : 'bg-[#FAFCFF] border-slate-200 text-slate-800 selection:bg-sky-200',
    ]"
  >
    <div
      class="absolute inset-0 pointer-events-none z-0 opacity-40 bg-grid-pattern transition-colors duration-300"
      style="background-size: 24px 24px"
    ></div>

    <section
      class="relative z-10 flex flex-col w-full h-full rounded-xl border m-1 md:m-2 shadow-2xl overflow-hidden backdrop-blur-md transition-colors duration-300"
      :class="
        isDarkMode
          ? 'bg-slate-800/70 border-slate-700'
          : 'bg-white/60 border-slate-200'
      "
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

      <main class="flex flex-1 overflow-hidden relative">
        <SettingsSidebar
          :isDarkMode="isDarkMode"
          :activeTab="activeTab"
          :tabs="tabs"
          @update:activeTab="activeTab = $event"
        />

        <section
          class="flex-1 p-6 md:p-8 overflow-y-auto relative z-10 scroll-smooth"
        >
          <transition name="fade-slide" mode="out-in">
            <PetTab
              v-if="activeTab === 'pet'"
              key="pet"
              :isDarkMode="isDarkMode"
              :petScale="petScale"
              :PET_SCALE_MIN="PET_SCALE_MIN"
              :PET_SCALE_MAX="PET_SCALE_MAX"
              @updateScale="updateScale"
              @resetScale="resetScale"
            />
            <HistoryTab
              v-else-if="activeTab === 'interaction'"
              key="interaction"
              :isDarkMode="isDarkMode"
            />
            <WindowTab
              v-else-if="activeTab == 'window'"
              key="window"
              :isDarkMode="isDarkMode"
            />
            <TodoTab v-else key="todo" :isDarkMode="isDarkMode" />
          </transition>
        </section>
      </main>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
// 替换为你项目中实际存在的 store 路径
import {
  PET_SCALE_DEFAULT,
  PET_SCALE_MAX,
  PET_SCALE_MIN,
  useSettingsStore,
} from "../../../stores/modules/settings";
import { useGameStore } from "../../../stores/modules/game";

// 引入 Lucide 图标
import {
  Ruler,
  Pointer,
  LayoutTemplate,
  Book,
  Cat,
  CheckCircle2,
} from "lucide-vue-next";

// 引入自定义组件
import SettingsHeader from "../components/SettingsHeader.vue";
import SettingsSidebar from "../components/SettingsSidebar.vue";
import { PetTab, HistoryTab, WindowTab } from "../components/tabs";
import TodoTab from "../components/tabs/TodoTab.vue";
import { useUIStore } from "../../../stores/modules/ui/ui";

const PET_SCALE_EVENT = "pet-scale-changed";
const DIALOG_HISTORY_EVENT = "dialog-history-changed";
const appWindow = getCurrentWindow();
const settingsStore = useSettingsStore();
const gameStore = useGameStore();
const uiStore = useUIStore();

const isMaximized = ref(false);
const activeTab = ref<"pet" | "interaction" | "window" | "todo">("pet");

// 深色模式状态与切换方法 (如需持久化可绑定到 settingsStore)
const isDarkMode = computed(() => uiStore.isDarkMode);
const toggleTheme = () => {
  uiStore.isDarkMode = !uiStore.isDarkMode;
};

type TabItem = {
  key: "pet" | "interaction" | "window" | "todo";
  label: string;
  icon: any;
  en: string;
};

const tabs = [
  { key: "pet", label: "桌宠体型", icon: Ruler, en: "SCALE CONFIG" } as TabItem,
  {
    key: "interaction",
    label: "历史对话",
    icon: Book,
    en: "HISTORY DIALOGUE",
  } as TabItem,
  {
    key: "todo",
    label: "待办事项",
    icon: CheckCircle2,
    en: "TODO LIST",
  } as TabItem,
  {
    key: "window",
    label: "主动对话",
    icon: Cat,
    en: "PROACTIVE SYSTEM",
  } as TabItem,
];

const petScale = computed(() => settingsStore.pet.scale);

const syncMaximizedState = async () => {
  isMaximized.value = await appWindow.isMaximized();
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

const minimizeWindow = async () => {
  await appWindow.minimize();
};

const toggleMaximizeWindow = async () => {
  await appWindow.toggleMaximize();
  await syncMaximizedState();
};

const closeWindow = async () => {
  await appWindow.close();
};

onMounted(async () => {
  await syncMaximizedState();

  // 监听Main窗口的dialogHistory变化
  const unlisten = await appWindow.listen<{ dialogHistory: any[] }>(
    DIALOG_HISTORY_EVENT,
    (event) => {
      const { dialogHistory } = event.payload;
      if (dialogHistory) {
        gameStore.dialogHistory = dialogHistory;
      }
    },
  );

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
