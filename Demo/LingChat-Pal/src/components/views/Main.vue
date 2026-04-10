<template>
  <div
    id="app"
    :style="appStyleVars"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
    class="relative w-(--app-width) h-(--app-height) flex flex-col justify-start items-center overflow-hidden transition-none"
  >
    <!-- DialogueBox 区域 -->
    <div
      class="w-full shrink-0 flex items-end justify-center transition-none"
      :style="{ height: 'var(--dialog-h)' }"
    >
      <DialogueBox />
    </div>

    <!-- Avatar 区域 -->
    <div
      class="shrink-0 flex items-center justify-center transition-all duration-100"
      :style="{ width: 'var(--avatar-size)', height: 'var(--avatar-size)' }"
    >
      <GameRolesStage
        @avatar-click="handleAvatarClick"
        @open-settings="handleOpenSettings"
      />
    </div>

    <!-- ChatInput 区域 -->
    <div
      class="w-full shrink-0 flex items-start justify-center transition-none"
      :style="{ height: 'var(--chat-h)' }"
    >
      <ChatInput :visible="showChatInput" @message-sent="handleMessageSent" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { getCurrentWindow, LogicalSize, Window } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useGameStore } from "../../stores/modules/game";
import { useSettingsStore } from "../../stores/modules/settings";
import { useUserStore } from "../../stores/modules/user/user";

import ChatInput from "../game/ChatInput.vue";
import DialogueBox from "../game/DialogueBox.vue";
import { eventQueue } from "../../core/events/event-queue";
import GameRolesStage from "../game/GameRolesStage.vue";

const PET_SCALE_EVENT = "pet-scale-changed";
const DIALOG_HISTORY_EVENT = "dialog-history-changed";

const BASE_AVATAR_SIZE = 240;
const CHAT_BASE_H = 45;
const DIALOG_BASE_H = 75;

const gameStore = useGameStore();
const settingsStore = useSettingsStore();
const userStore = useUserStore();

const mainWindow = ref<Window | null>(null);
const showChatInput = ref(false);

const appStyleVars = computed(() => {
  const scale = settingsStore.pet.scale || 1;
  const layout = calcWindowLayout(scale);
  return {
    "--pet-ui-scale": scale.toString(),
    "--app-width": `${layout.width}px`,
    "--app-height": `${layout.height}px`,
    "--avatar-size": `${Math.round(BASE_AVATAR_SIZE * scale)}px`,
    "--chat-h": `${Math.round(CHAT_BASE_H * scale)}px`,
    "--dialog-h": `${Math.round(DIALOG_BASE_H * scale)}px`,
  };
});

const calcWindowLayout = (scale: number): { width: number; height: number } => {
  const S = Math.round(BASE_AVATAR_SIZE * scale);
  const chatH = Math.round(CHAT_BASE_H * scale);
  const dialogH = Math.round(DIALOG_BASE_H * scale);
  return { width: S, height: S + dialogH + chatH }; // 固定总大小包裹三个元素的最大范围
};

const runInitialization = async () => {
  const userId = "1"; // TODO: 获取真实 userId
  try {
    await gameStore.initializeGame(userStore.client_id, userId);
  } catch (error) {
    console.log(error);
  }
};

const applyWindowLayout = async () => {
  if (!mainWindow.value) return;
  try {
    const scale = settingsStore.pet.scale || 1;
    const layout = calcWindowLayout(scale);

    // 只在尺度改变时更新大小时，且无需调整偏置(坐标不再偏移)
    await mainWindow.value.setSize(
      new LogicalSize(layout.width, layout.height),
    );
  } catch (error) {
    console.error("调整窗口布局失败:", error);
  }
};

const openSettingsWindow = async () => {
  const existingWindow = await WebviewWindow.getByLabel("settings");
  if (existingWindow) {
    try {
      await existingWindow.unminimize();
      await existingWindow.setFocus();
    } catch (error) {
      console.error("激活设置窗口失败:", error);
    }
    return;
  }

  const isDev = Boolean((import.meta as any).env?.DEV);
  const targetUrl = isDev
    ? `${window.location.origin}#/second`
    : "index.html#/second";

  const webview = new WebviewWindow("settings", {
    url: targetUrl,
    title: "设置",
    width: 1100,
    height: 760,
    minWidth: 860,
    minHeight: 620,
    resizable: true,
    shadow: false,
    decorations: false,
    transparent: true,
    alwaysOnTop: false,
    visible: true,
  });

  webview.once("tauri://error", (e) => {
    console.error("创建设置窗口失败:", e);
  });
};

let unlistenScaleEvent: (() => void) | null = null;
let unlistenDialogHistoryEvent: (() => void) | null = null;

onMounted(async () => {
  mainWindow.value = getCurrentWindow();

  // Set initial sizes statically for the pet bounding box
  await applyWindowLayout();

  unlistenScaleEvent = await mainWindow.value.listen<{ scale: number }>(
    PET_SCALE_EVENT,
    async (event) => {
      const scale = Number(event.payload?.scale);
      if (!Number.isNaN(scale)) {
        settingsStore.pet.scale = scale;
        await applyWindowLayout();
      }
    },
  );
});

watch(
  () => userStore.client_id,
  (newId) => {
    if (newId) {
      runInitialization();
    }
  },
);

watch(
  () => settingsStore.pet.scale,
  () => {
    void applyWindowLayout();
  },
);

// 监听dialogHistory变化，通知SettingsPage窗口
watch(
  () => gameStore.dialogHistory,
  (newHistory) => {
    if (mainWindow.value) {
      void mainWindow.value.emit(DIALOG_HISTORY_EVENT, {
        dialogHistory: newHistory,
      });
    }
  },
  { deep: true },
);

onUnmounted(() => {
  if (unlistenScaleEvent) {
    unlistenScaleEvent();
    unlistenScaleEvent = null;
  }
  if (unlistenDialogHistoryEvent) {
    unlistenDialogHistoryEvent();
    unlistenDialogHistoryEvent = null;
  }
});

const handleMessageSent = (message: string) => {
  console.log("Main: 消息已发送:", message);
};

const handleMouseEnter = () => {
  showChatInput.value = true;
};

const handleMouseLeave = () => {
  showChatInput.value = false;
};

const handleAvatarClick = () => {
  eventQueue.continue();
};

const handleOpenSettings = () => {
  void openSettingsWindow();
};
</script>

<style scoped>
#app {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
}
</style>
