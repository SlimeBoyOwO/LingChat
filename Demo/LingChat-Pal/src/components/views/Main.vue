<template>
  <div
    id="app"
    :style="appStyleVars"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
    class="relative w-full h-full flex flex-col justify-start items-center overflow-hidden transition-none"
  >
    <!-- DialogueBox 区域 -->
    <div
      class="w-[90%] shrink-0 flex items-end justify-center transition-none pb-2"
      :style="{
        height: 'var(--dialog-h)',
        display: domLayout.dialogVisible ? 'flex' : 'none',
      }"
    >
      <DialogueBox />
    </div>

    <!-- Avatar 区域 -->
    <div
      class="shrink-0 flex items-center justify-center transition-none"
      :style="{ width: 'var(--avatar-size)', height: 'var(--avatar-size)' }"
    >
      <GameRolesStage
        @avatar-click="handleAvatarClick"
        @open-settings="handleOpenSettings"
      />
    </div>

    <!-- ChatInput 区域 -->
    <div
      class="w-[90%] shrink-0 flex items-start justify-center transition-none pt-2"
      :style="{
        height: 'var(--chat-h)',
        display: domLayout.chatVisible ? 'flex' : 'none',
      }"
    >
      <ChatInput
        :visible="domLayout.chatVisible"
        @message-sent="handleMessageSent"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch, nextTick } from "vue";
import {
  getCurrentWindow,
  LogicalSize,
  LogicalPosition,
  Window,
} from "@tauri-apps/api/window";
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

const BASE_AVATAR_SIZE = 280;
const CHAT_BASE_H = 60;
const DIALOG_BASE_H = 80;

type ExpectedLayout = {
  width: number;
  height: number;
  offsetX: number;
  offsetY: number;
  avatarX: number;
  avatarY: number;
  chatVisible: boolean;
  dialogVisible: boolean;
};

const mainWindow = ref<Window | null>(null);
const gameStore = useGameStore();
const settingsStore = useSettingsStore();
const userStore = useUserStore();
const showChatInput = ref(false);

let unlistenScaleEvent: (() => void) | null = null;
let unlistenDialogHistoryEvent: (() => void) | null = null;
let isApplyingWindowLayout = false;
let hasPendingWindowLayout = false;
let lastAppliedLayout: ExpectedLayout | null = null;

const domLayout = ref<ExpectedLayout>({
  width: BASE_AVATAR_SIZE,
  height: BASE_AVATAR_SIZE,
  offsetX: 0,
  offsetY: 0,
  avatarX: 0,
  avatarY: 0,
  chatVisible: false,
  dialogVisible: false,
});

const dialogueVisible = computed(() => {
  return (
    gameStore.currentStatus === "responding" &&
    gameStore.currentLine.trim().length > 0
  );
});

const appStyleVars = computed(() => {
  const scale = settingsStore.pet.scale || 1;
  return {
    "--pet-ui-scale": scale.toString(),
    "--app-width": `${domLayout.value.width}px`,
    "--app-height": `${domLayout.value.height}px`,
    "--avatar-x": `${domLayout.value.avatarX}px`,
    "--avatar-y": `${domLayout.value.avatarY}px`,
    "--avatar-size": `${Math.round(BASE_AVATAR_SIZE * scale)}px`,
    "--chat-h": `${Math.round(CHAT_BASE_H * scale)}px`,
    "--dialog-h": `${Math.round(DIALOG_BASE_H * scale)}px`,
  };
});

const calcWindowLayout = (
  scale: number,
  showChat: boolean,
  showDialog: boolean,
): ExpectedLayout => {
  const S = Math.round(BASE_AVATAR_SIZE * scale);
  const chatH = Math.round(CHAT_BASE_H * scale);
  const dialogH = Math.round(DIALOG_BASE_H * scale);

  const W = S; // 保持宽度始终等于正方形大小，避免水平发生变化引起 IPC 时差带来的抽搐
  const H = S + (showChat ? chatH : 0) + (showDialog ? dialogH : 0);

  const offsetX = 0;
  const offsetY = showDialog ? -dialogH : 0;

  return {
    width: W,
    height: H,
    offsetX,
    offsetY,
    avatarX: 0,
    avatarY: showDialog ? dialogH : 0,
    chatVisible: showChat,
    dialogVisible: showDialog,
  };
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
  if (!mainWindow.value) {
    return;
  }

  if (isApplyingWindowLayout) {
    hasPendingWindowLayout = true;
    return;
  }

  isApplyingWindowLayout = true;
  try {
    const scale = settingsStore.pet.scale || 1;
    const layout = calcWindowLayout(
      scale,
      showChatInput.value,
      dialogueVisible.value,
    );

    if (lastAppliedLayout) {
      if (
        lastAppliedLayout.width === layout.width &&
        lastAppliedLayout.height === layout.height &&
        lastAppliedLayout.offsetX === layout.offsetX &&
        lastAppliedLayout.offsetY === layout.offsetY
      ) {
        domLayout.value = layout;
        return;
      }

      const deltaX = layout.offsetX - lastAppliedLayout.offsetX;
      const deltaY = layout.offsetY - lastAppliedLayout.offsetY;

      // 如果窗口变小（例如对话框关闭，deltaY 会从 -H 变成 0，即 deltaY > 0），
      // 我们必须“先缩减 DOM 高度”，这样底层原生窗口缩下去时元素的相对上边缘就不会超过界限导致闪屏。
      const isShrinking =
        deltaY > 0 || layout.height < lastAppliedLayout.height;

      if (isShrinking) {
        domLayout.value = layout;
        await nextTick(); // 给 Vue 渲染时间，让 DOM 先抽离
      }

      if (deltaX !== 0 || deltaY !== 0) {
        const factor = await mainWindow.value.scaleFactor();
        const pos = await mainWindow.value.outerPosition();
        const logicalPos = pos.toLogical(factor);
        await mainWindow.value.setPosition(
          new LogicalPosition(
            Math.round(logicalPos.x + deltaX),
            Math.round(logicalPos.y + deltaY),
          ),
        );
      }

      await mainWindow.value.setSize(
        new LogicalSize(layout.width, layout.height),
      );

      // 如果是放大窗口（例如展开对话框），则原生窗口先放大、往上拉，然后 DOM 瞬间出现，避免被推下
      if (!isShrinking) {
        domLayout.value = layout;
      }
    } else {
      await mainWindow.value.setSize(
        new LogicalSize(layout.width, layout.height),
      );
      domLayout.value = layout;
    }

    lastAppliedLayout = layout;
  } catch (error) {
    console.error("调整窗口布局失败:", error);
  } finally {
    isApplyingWindowLayout = false;
    if (hasPendingWindowLayout) {
      hasPendingWindowLayout = false;
      void applyWindowLayout();
    }
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

onMounted(async () => {
  mainWindow.value = getCurrentWindow();

  // Set initial applied layout without moving the physical window position
  const scale = settingsStore.pet.scale || 1;
  const initialLayout = calcWindowLayout(
    scale,
    showChatInput.value,
    dialogueVisible.value,
  );
  await mainWindow.value.setSize(
    new LogicalSize(initialLayout.width, initialLayout.height),
  );
  lastAppliedLayout = initialLayout;
  domLayout.value = initialLayout;

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

watch([showChatInput, dialogueVisible], () => {
  void applyWindowLayout();
});

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
