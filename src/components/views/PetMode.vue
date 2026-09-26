<template>
  <!--
    宠物窗 = 锚点窗口：只有头像带与输入带，顶边就是头像顶边，因此贴得住屏幕最顶。
    气泡/通知不在本窗口内，见 BubbleWindow.vue。
  -->
  <div
    id="pet-app"
    :style="appStyleVars"
    class="relative flex h-(--app-height) w-(--app-width) flex-col items-center overflow-hidden bg-transparent transition-none select-none"
  >
    <DragArea :isDragging="isDragging">
      <div
        ref="avatarContainer"
        class="flex shrink-0 items-center justify-center bg-transparent"
        :style="{ width: 'var(--avatar-size)', height: 'var(--avatar-size)' }"
      >
        <GameRolesStage
          @avatar-click="handleAvatarClick"
          @open-settings="handleOpenSettings"
          @switch-auto-mode="handleSwitchAutoMode"
          @exit-pet-mode="handleExitPetMode"
          @audio-ended="handleAudioFinished"
          @audio-started="handleAudioStarted"
        />
      </div>
    </DragArea>

    <div
      ref="chatContainer"
      class="flex w-full shrink-0 items-start justify-center bg-transparent transition-none"
      :style="{ height: 'var(--chat-h)' }"
    >
      <ChatInput ref="ChatInputRef" :visible="showChatInput" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { useGameStore } from "@/stores/modules/game";
import { useSettingsStore } from "@/stores/modules/settings";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useAutoAdvance } from "@/composables/chat/useAutoAdvance";
import { eventQueue } from "@/core/events/event-queue";

import ChatInput from "../pet/ChatInput.vue";
import DragArea from "../pet/DragArea.vue";
import GameRolesStage from "../pet/GameRolesStage.vue";
import { useFileDrop } from "../pet/useFileDrop";
import {
  AVATAR_BAND_BASE,
  CHAT_BASE_H,
  PET_WINDOW_H_BASE,
  WINDOW_WIDTH_BASE,
} from "../pet/constants";
import { PET_BUBBLE_EVENT, PET_BUBBLE_REQUEST, type BubbleMirror } from "../pet/bubbleMirror";

const { t } = useI18n();
const router = useRouter();
const gameStore = useGameStore();
const settingsStore = useSettingsStore();
const uiStore = useUIStore();

const showChatInput = ref(false);
const { isDragging } = useFileDrop();

const avatarContainer = ref<HTMLElement | null>(null);
const chatContainer = ref<HTMLElement | null>(null);
const ChatInputRef = ref<InstanceType<typeof ChatInput> | null>(null);

const scale = computed(() => settingsStore.pet?.scale || 1);
const px = (base: number) => `${Math.round(base * scale.value)}px`;

const appStyleVars = computed(() => ({
  "--pet-ui-scale": scale.value.toString(),
  "--app-width": px(WINDOW_WIDTH_BASE),
  "--app-height": px(PET_WINDOW_H_BASE),
  "--avatar-size": px(AVATAR_BAND_BASE),
  "--chat-h": px(CHAT_BASE_H),
}));

// ─────────────────────────── 镜像显示状态给气泡窗 ───────────────────────────

const appWindow = getCurrentWindow();

const emitMirror = () => {
  const payload: BubbleMirror = {
    status: gameStore.currentStatus,
    line: uiStore.showCharacterLine,
    title: uiStore.showCharacterTitle,
    subtitle: uiStore.showCharacterSubtitle,
    emotion: uiStore.showCharacterEmotion,
    motionText: uiStore.showCharacterMotionText,
    textSpeed: uiStore.typeWriterSpeed,
    petScale: scale.value,
    notification: {
      isVisible: uiStore.notification.isVisible,
      title: uiStore.notification.title,
      message: uiStore.notification.message,
      type: uiStore.notification.type,
    },
  };
  void appWindow.emitTo("pet_bubble", PET_BUBBLE_EVENT, payload);
};

// ─────────────────────────── 窗口与命中区 ───────────────────────────

const applyWindowLayout = async () => {
  try {
    await invoke("set_pet_mode", { enable: true, scale: scale.value });
  } catch (error) {
    console.error("调整窗口布局失败:", error);
  }
};

let lastRectsKey = "";
const reportSolidRegions = () => {
  const rects: { x: number; y: number; width: number; height: number }[] = [];

  if (avatarContainer.value) {
    const r = avatarContainer.value.getBoundingClientRect();
    rects.push({ x: r.x, y: r.y, width: r.width, height: r.height });
  }
  // 输入框显示时略微外扩，保证极小尺寸下的判定连贯
  if (chatContainer.value && showChatInput.value) {
    const r = chatContainer.value.getBoundingClientRect();
    rects.push({ x: r.x - 20, y: r.y - 20, width: r.width + 40, height: r.height + 40 });
  }

  const rectsKey = JSON.stringify(rects);
  if (rectsKey === lastRectsKey) return;
  lastRectsKey = rectsKey;
  invoke("update_solid_regions", { rects }).catch(() => {
    lastRectsKey = "";
  });
};

const unlisteners: (() => void)[] = [];
let hitTestInterval: number | undefined;

onMounted(async () => {
  document.body.style.backgroundColor = "transparent";
  document.documentElement.style.backgroundColor = "transparent";

  // 气泡窗启动晚于本窗口，挂载后会主动请求一次当前状态
  unlisteners.push(
    await appWindow.listen(PET_BUBBLE_REQUEST, emitMirror),
    // 桌宠设置窗口的四个热更通道：设置窗口只改自己的 store + 广播，本窗口负责生效
    await appWindow.listen<{ scale: number }>("pet-scale-changed", (event) => {
      const next = Number(event.payload?.scale);
      if (!Number.isNaN(next)) settingsStore.pet.scale = next;
    }),
    await appWindow.listen<{ fps: number }>("pet-live2d-fps-changed", (event) => {
      const next = Number(event.payload?.fps);
      if (!Number.isNaN(next)) settingsStore.setPetLive2dFps(next);
    }),
    await appWindow.listen<{ volume: number }>("pet-volume-changed", (event) => {
      const next = Number(event.payload?.volume);
      if (!Number.isNaN(next)) settingsStore.updateAudio({ characterVolume: next });
    }),
    await appWindow.listen<{ effect: string }>("background-effect-changed", (event) => {
      if (event.payload?.effect) uiStore.setBackgroundEffect(event.payload.effect);
    }),
    // 输入框显隐兜底：光标离开 solid 区域后窗口会开启点击穿透，webview 从此收不到
    // 鼠标事件，mouseleave 可能永远不来（见 api/pet.rs::spawn_hit_test_poll）。
    await appWindow.listen<{ x: number; y: number }>("pet:cursor", (event) => {
      const { x, y } = event.payload;
      setShowChatInput(x >= 0 && y >= 0 && x <= window.innerWidth && y <= window.innerHeight);
    }),
  );

  await applyWindowLayout();
  hitTestInterval = window.setInterval(reportSolidRegions, 100);
});

onUnmounted(() => {
  document.body.style.backgroundColor = "";
  document.documentElement.style.backgroundColor = "";
  unlisteners.forEach((unlisten) => unlisten());
  if (hitTestInterval !== undefined) window.clearInterval(hitTestInterval);
});

watch(scale, () => void applyWindowLayout());

// 显示状态变化即推给气泡窗（气泡窗不参与状态机，只复现这些字段）
watch(
  () => [
    uiStore.showCharacterLine,
    uiStore.showCharacterEmotion,
    gameStore.currentStatus,
    uiStore.notification.isVisible,
    uiStore.notification.message,
    scale.value,
  ],
  emitMirror,
);

// ─────────────────────────────── 交互 ───────────────────────────────

/** 光标在桌宠窗口内就显示输入框，离开则隐藏；草稿非空（正在打字）时保持显示 */
const setShowChatInput = (insideWindow: boolean) => {
  showChatInput.value = insideWindow || (ChatInputRef.value?.isTyping() ?? false);
};

const handleMouseEnter = () => setShowChatInput(true);
const handleMouseLeave = () => setShowChatInput(false);

/** 推进对话：气泡在另一个窗口，故直接驱动事件队列 */
const handleAvatarClick = () => eventQueue.continue();

const handleOpenSettings = async () => {
  try {
    const existing = await WebviewWindow.getByLabel("settings");
    if (existing) {
      await existing.setFocus();
      return;
    }

    new WebviewWindow("settings", {
      url: "/second",
      title: t("views.petMode.settingsWindowTitle"),
      width: 1200,
      height: 800,
      resizable: true,
      shadow: false,
      decorations: false,
      transparent: true,
      alwaysOnTop: false,
    }).once("tauri://error", (e) => console.error("创建设置窗口失败:", e));
  } catch (error) {
    console.error("打开设置窗口时出错:", error);
  }
};

// 自动推进与语音收尾仍在本窗口调度；气泡是显示层，故没有对话组件句柄
const {
  onAudioStarted: handleAudioStarted,
  onAudioFinished: handleAudioFinished,
  toggleAutoMode: handleSwitchAutoMode,
} = useAutoAdvance({ dialog: () => null, mergeEnabled: false });

const handleExitPetMode = async () => {
  try {
    const settingsWindow = await WebviewWindow.getByLabel("settings");
    if (settingsWindow) await settingsWindow.close();
  } catch {
    // 窗口不存在，忽略
  }

  await invoke("update_solid_regions", { rects: [] });
  await invoke("set_pet_mode", { enable: false });
  router.push("/chat");
};

watch(
  () => gameStore.dialogHistory.length,
  () => {
    appWindow.emit("dialog-history-changed", {
      dialogHistory: JSON.parse(JSON.stringify(gameStore.dialogHistory)),
    });
  },
);
</script>

<style scoped>
#pet-app {
  position: relative;
  width: 100vw;
  height: 100dvh;
  overflow: hidden;
}
</style>
