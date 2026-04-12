<template>
  <div id="app" :style="appStyleVars" @mouseenter="handleMouseEnter" @mouseleave="handleMouseLeave"
    class="relative w-(--app-width) h-(--app-height) flex flex-col justify-start items-center overflow-hidden transition-none">
    <!-- DialogueBox 区域 -->
    <div ref="dialogContainer" class="w-full shrink-0 flex items-end justify-center transition-none"
      :style="{ height: 'var(--dialog-h)' }">
      <DialogueBox ref="gameDialogRef" @player-continued="manualTriggerContinue" @dialog-proceed="resetInteraction" />
    </div>

    <!-- Avatar 区域 -->
    <div ref="avatarContainer" class="shrink-0 flex items-center justify-center transition-all duration-100"
      :style="{ width: 'var(--avatar-size)', height: 'var(--avatar-size)' }">
      <GameRolesStage @avatar-click="handleAvatarClick" @open-settings="handleOpenSettings"
        @switch-auto-mode="handleSwitchAutoMode" @audio-ended="handleAudioFinished" @audio-started="handleAudioStarted"
        @player-continued="manualTriggerContinue" />
    </div>

    <!-- ChatInput 区域 -->
    <div ref="chatContainer" class="w-full shrink-0 flex items-start justify-center transition-none"
      :style="{ height: 'var(--chat-h)' }">
      <ChatInput :visible="showChatInput" @message-sent="handleMessageSent" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { getCurrentWindow, LogicalSize, Window } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useGameStore } from "../../stores/modules/game";
import { useSettingsStore } from "../../stores/modules/settings";
import { useUserStore } from "../../stores/modules/user/user";

import ChatInput from "../game/ChatInput.vue";
import DialogueBox from "../game/DialogueBox.vue";
import { eventQueue } from "../../core/events/event-queue";
import GameRolesStage from "../game/GameRolesStage.vue";
import { useUIStore } from "../../stores/modules/ui/ui";

const PET_SCALE_EVENT = "pet-scale-changed";
const DIALOG_HISTORY_EVENT = "dialog-history-changed";

const BASE_AVATAR_SIZE = 240;
const CHAT_BASE_H = 45;
const DIALOG_BASE_H = 75;

const gameStore = useGameStore();
const settingsStore = useSettingsStore();
const userStore = useUserStore();
const uiStore = useUIStore();

const mainWindow = ref<Window | null>(null);
const showChatInput = ref(false);

const dialogContainer = ref<HTMLElement | null>(null);
const avatarContainer = ref<HTMLElement | null>(null);
const chatContainer = ref<HTMLElement | null>(null);
const gameDialogRef = ref<InstanceType<typeof DialogueBox> | null>(null);

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
let unlistenSettingsEvent: UnlistenFn | null = null;
let unlistenBackgroundEffectEvent: (() => void) | null = null;
let hitTestInterval: number | undefined;

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

  unlistenBackgroundEffectEvent = await mainWindow.value.listen<{
    effect: string;
  }>("background-effect-changed", (event) => {
    const effect = event.payload?.effect;
    if (effect) {
      uiStore.setBackgroundEffect(effect);
    }
  });

  unlistenSettingsEvent = await listen("open-settings", () => {
    handleOpenSettings();
  });

  hitTestInterval = window.setInterval(() => {
    const rects = [];
    if (
      dialogContainer.value &&
      gameStore.currentStatus === "responding" &&
      gameStore.currentLine.trim() !== ""
    ) {
      const r = dialogContainer.value.getBoundingClientRect();
      rects.push({ x: r.x, y: r.y, width: r.width, height: r.height });
    }
    if (avatarContainer.value) {
      const r = avatarContainer.value.getBoundingClientRect();
      rects.push({ x: r.x, y: r.y, width: r.width, height: r.height });
    }
    if (chatContainer.value && showChatInput.value) {
      const r = chatContainer.value.getBoundingClientRect();
      // Expand chat input slightly to prevent gaps dropping interactions
      rects.push({
        x: r.x - 20,
        y: r.y - 20,
        width: r.width + 40,
        height: r.height + 40,
      });
    }
    invoke("update_solid_regions", { rects }).catch(console.error);
  }, 100);
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
  if (unlistenSettingsEvent) {
    unlistenSettingsEvent();
    unlistenSettingsEvent = null;
  }
  if (unlistenBackgroundEffectEvent) {
    unlistenBackgroundEffectEvent();
    unlistenBackgroundEffectEvent = null;
  }
  if (hitTestInterval !== undefined) {
    window.clearInterval(hitTestInterval);
  }
});

const handleMessageSent = (message: string) => {
  gameStore.appendGameMessage({
    type: "message",
    displayName: gameStore.userName,
    content: message,
  });
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

let timerId: any = null;
// 2. 状态标志，记录 continue() 是否已被调用
const isContinueTriggered = ref(false);
// 3. 追踪音频和打字状态
const audioFinished = ref(true); // 默认 true（无音频时视为已完成）

// 在新交互开始前调用的重置函数
const resetInteraction = () => {
  isContinueTriggered.value = false;
  audioFinished.value = true;
  if (timerId) {
    clearTimeout(timerId);
    timerId = null;
  }
};

// 尝试触发自动继续（打字和音频都结束后才执行）
const tryAutoAdvance = () => {
  if (!uiStore.autoMode) return;
  if (isContinueTriggered.value) return;
  if (gameStore.currentStatus !== "responding") return;

  const typing = gameDialogRef.value?.isTyping ?? false;
  if (typing || !audioFinished.value) return;

  if (timerId) clearTimeout(timerId);
  timerId = setTimeout(() => {
    if (gameDialogRef.value) {
      const needWait = gameDialogRef.value.continueDialog(false);
      if (needWait) {
        tryAutoAdvance();
      }
    }
  }, 1000);
};

// 音频开始播放
const handleAudioStarted = () => {
  audioFinished.value = false;
};

// 音频播放结束
const handleAudioFinished = () => {
  audioFinished.value = true;
  tryAutoAdvance();
};

// 监听打字结束
watch(
  () => gameDialogRef.value?.isTyping,
  (typing) => {
    console.log("父组件：打字状态变化", typing);
    if (typing === false) {
      tryAutoAdvance();
    }
  },
);

// 用户手动触发的函数
const manualTriggerContinue = () => {
  console.log("用户主动点击了");
  if (timerId) {
    clearTimeout(timerId);
    timerId = null;
  }

  if (!isContinueTriggered.value) {
    isContinueTriggered.value = true;
  } else {
  }
};

const handleSwitchAutoMode = () => {
  uiStore.autoMode = !uiStore.autoMode;
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
