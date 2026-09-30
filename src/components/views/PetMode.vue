<template>
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
      class="flex w-full shrink-0 items-end justify-center bg-transparent transition-none"
      :style="{ height: 'var(--chat-h)', paddingBottom: px(CHAT_BASE_PB) }"
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
import { getCurrentWindow, currentMonitor } from "@tauri-apps/api/window";

import { useGameStore } from "@/stores/modules/game";
import { useSettingsStore } from "@/stores/modules/settings";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useAutoAdvance } from "@/composables/chat/useAutoAdvance";
import { useDialogAdvance } from "@/composables/chat/useDialogAdvance";

import ChatInput from "../pet/ChatInput.vue";
import DragArea from "../pet/DragArea.vue";
import GameRolesStage from "../pet/GameRolesStage.vue";
import { useFileDrop } from "../pet/useFileDrop";
import {
  AVATAR_BAND_BASE,
  BUBBLE_GAP_BASE,
  BUBBLE_WINDOW_H_BASE,
  CHAT_BASE_H,
  CHAT_BASE_PB,
  PET_WINDOW_H_BASE,
  WINDOW_WIDTH_BASE,
} from "../pet/constants";
import {
  PET_BUBBLE_EVENT,
  PET_BUBBLE_REQUEST,
  PET_FINISH_TYPING_EVENT,
  PET_LINE_DRAINED,
  nextLineId,
  type BubbleAlign,
  type BubbleMirror,
  type BubbleSide,
  type LineDrainedPayload,
} from "../pet/bubbleMirror";

const { t } = useI18n();
const router = useRouter();
const gameStore = useGameStore();
const settingsStore = useSettingsStore();
const uiStore = useUIStore();

const showChatInput = ref(false);
const { isDragging } = useFileDrop();

/**
 * 「当前这句吐完了没有」——不镜像气泡窗的打字机状态，而是本地相减得出。
 *
 * 打字机在气泡窗，跨窗口读一个会变的布尔量只能靠变化边沿，丢一次就永久失联。
 * 改成气泡窗上报「第 N 句已完整显示」：未上报的当前句即视为还在打字。
 */
const currentLineId = ref(nextLineId());
const drainedLineId = ref(currentLineId.value);
const lineTyping = computed(() => drainedLineId.value !== currentLineId.value);

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

const PET_BUBBLE_SIDE_EVENT = "pet-bubble-side-changed";

const appWindow = getCurrentWindow();

/** 上一次镜像出去的台词，用于判断「这是新的一句」——见 emitMirror 里的发号 */
let lastMirroredLine = "";

const emitMirror = () => {
  const line = uiStore.showCharacterLine;
  // 惰性发号：只能在镜像处推进，放独立 watch 里会因注册顺序让本次 mirror 带上上一句的号
  if (line.trim() !== "" && line !== lastMirroredLine) {
    lastMirroredLine = line;
    currentLineId.value = nextLineId();
    // 新句立刻压住调度：气泡那边的完成信号要跨进程绕一圈才回来
    typingFinished.value = false;
    cancelAdvance();
  }
  const payload: BubbleMirror = {
    status: gameStore.currentStatus,
    line,
    lineId: currentLineId.value,
    title: uiStore.showCharacterTitle,
    subtitle: uiStore.showCharacterSubtitle,
    emotion: uiStore.showCharacterEmotion,
    motionText: uiStore.showCharacterMotionText,
    textSpeed: uiStore.typeWriterSpeed,
    // 气泡窗不跑事件处理器：语音字段不镜像的话，有角色语音时也会播打字音效
    avatarAudio: uiStore.currentAvatarAudio,
    petScale: scale.value,
    bubbleSide: mirroredSide.value,
    bubbleAlign: mirroredAlign.value,
    // 贴边出屏的溢出量：让气泡窗把内容拉回屏幕内
    alignInset: bubbleAlignInset.value,
    swapping: swapping.value,
    notification: {
      isVisible: uiStore.notification.isVisible,
      title: uiStore.notification.title,
      message: uiStore.notification.message,
      type: uiStore.notification.type,
    },
  };
  void appWindow.emitTo("pet_bubble", PET_BUBBLE_EVENT, payload);
};

const petLeftCss = ref(0);
const petTopCss = ref(0);
const petWidthCss = ref(WINDOW_WIDTH_BASE);
const petHeightCss = ref(PET_WINDOW_H_BASE);
const workLeftCss = ref(0);
const workTopCss = ref(0);
const workRightCss = ref(0);
const workBottomCss = ref(0);
const layoutReady = ref(false);
let petDpr = 1;
const swapping = ref(false);
const mirroredSide = ref<BubbleSide>("above");
const mirroredAlign = ref<BubbleAlign>("top");

const bubbleSideSetting = computed(() => settingsStore.pet?.bubbleSide ?? "above");

const sideRooms = computed(() => {
  const gap = BUBBLE_GAP_BASE * scale.value;
  const bubbleW = WINDOW_WIDTH_BASE * scale.value;
  const bubbleH = BUBBLE_WINDOW_H_BASE * scale.value;
  return {
    above: petTopCss.value - workTopCss.value - bubbleH - gap,
    below: workBottomCss.value - (petTopCss.value + petHeightCss.value) - bubbleH - gap,
    left: petLeftCss.value - workLeftCss.value - bubbleW - gap,
    right: workRightCss.value - (petLeftCss.value + petWidthCss.value) - bubbleW - gap,
  };
});

// 自动选边阈值（基准值，实际都乘 pet.scale）
const ABOVE_MIN_ROOM_BASE = 88; // 上方至少留这么多才继续上置 ≈ 单行台词 + 长尾 + 间隙
const CORNER_BAND_BASE = 32; // 头像离屏幕边多近才算「贴住这条边」
const HYSTERESIS_BASE = 48; // 反向切换多让出这么多，避免停在边界上反复横跳
const AVATAR_INSET_X_BASE = (WINDOW_WIDTH_BASE - AVATAR_BAND_BASE) / 2;

// 默认始终上置；只有缩在屏幕角上才侧置，上方连单行台词都放不下才下置
const bubbleSide = computed<BubbleSide>(() => {
  const mode = bubbleSideSetting.value;
  if (mode !== "auto") return mode;
  if (!layoutReady.value) return "above";

  const s = scale.value;
  const current = mirroredSide.value;
  const sidePlaced = current === "left" || current === "right";
  const hysteresis = HYSTERESIS_BASE * s;

  const rooms = sideRooms.value;
  const sideFits = rooms.right >= 0 || rooms.left >= 0;
  const roomierSide: BubbleSide = rooms.right >= rooms.left ? "right" : "left";

  const avatarLeft = petLeftCss.value + AVATAR_INSET_X_BASE * s;
  const avatarRight = petLeftCss.value + petWidthCss.value - AVATAR_INSET_X_BASE * s;
  const avatarTop = petTopCss.value;
  const avatarBottom = petTopCss.value + AVATAR_BAND_BASE * s;
  const band = (CORNER_BAND_BASE + (sidePlaced ? HYSTERESIS_BASE : 0)) * s;
  const atLeftEdge = avatarLeft - workLeftCss.value <= band;
  const atRightEdge = workRightCss.value - avatarRight <= band;
  const atTopEdge = avatarTop - workTopCss.value <= band;
  const atBottomEdge = workBottomCss.value - avatarBottom <= band;

  if ((atLeftEdge || atRightEdge) && (atTopEdge || atBottomEdge) && sideFits) return roomierSide;

  const aboveRoom = petTopCss.value - workTopCss.value;
  const aboveMinRoom = ABOVE_MIN_ROOM_BASE * s + (current === "above" ? 0 : hysteresis);
  if (aboveRoom < aboveMinRoom) {
    if (rooms.below >= 0) return "below";
    if (sideFits) return roomierSide;
    const all: BubbleSide[] = ["above", "below", "left", "right"];
    return all.reduce(
      (best, side) => (rooms[side] > rooms[best] ? side : best),
      "above" as BubbleSide,
    );
  }

  return "above";
});

const ALIGN_HYSTERESIS_CSS = 40;
const bubbleAlign = ref<BubbleAlign>("top");
const refreshBubbleAlign = () => {
  if (!layoutReady.value) return;
  const delta =
    petTopCss.value + petHeightCss.value / 2 - (workTopCss.value + workBottomCss.value) / 2;
  if (bubbleAlign.value === "top") {
    if (delta > ALIGN_HYSTERESIS_CSS) bubbleAlign.value = "bottom";
  } else if (delta < -ALIGN_HYSTERESIS_CSS) {
    bubbleAlign.value = "top";
  }
};

// 贴边跑到工作区外多少，就让气泡内容往里缩多少
const bubbleAlignInset = computed(() => {
  if (bubbleAlign.value === "bottom") {
    return Math.max(0, petTopCss.value + petHeightCss.value - workBottomCss.value);
  }
  return Math.max(0, workTopCss.value - petTopCss.value);
});

const syncBubblePlacement = async () => {
  try {
    const [pos, size, monitor] = await Promise.all([
      appWindow.outerPosition(),
      appWindow.outerSize(),
      currentMonitor(),
    ]);
    if (!monitor) return;
    const dpr = monitor.scaleFactor || window.devicePixelRatio || 1;
    petDpr = dpr;
    petLeftCss.value = pos.x / dpr;
    petTopCss.value = pos.y / dpr;
    if (size.width > 0) petWidthCss.value = size.width / dpr;
    if (size.height > 0) petHeightCss.value = size.height / dpr;
    workLeftCss.value = monitor.workArea.position.x / dpr;
    workTopCss.value = monitor.workArea.position.y / dpr;
    workRightCss.value = (monitor.workArea.position.x + monitor.workArea.size.width) / dpr;
    workBottomCss.value = (monitor.workArea.position.y + monitor.workArea.size.height) / dpr;
    layoutReady.value = true;
    refreshBubbleAlign();
  } catch {
    return;
  }
};

let placementTimer: number | undefined;
const schedulePlacementSync = () => {
  if (placementTimer !== undefined) window.clearTimeout(placementTimer);
  placementTimer = window.setTimeout(() => {
    void syncBubblePlacement().then(() => applyBubblePlacement(true));
  }, 120);
};

const pushBubbleSide = async (side: BubbleSide, align: BubbleAlign) => {
  try {
    await invoke("set_bubble_side", { side, align });
  } catch (error) {
    console.error("设置气泡位置失败:", error);
  }
};

const delay = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

// 换位：淡出 → 挪窗 → 淡入（独立窗口跳位是瞬时的，不遮住会闪一下）
let swapQueue: Promise<void> = Promise.resolve();
const runSwap = (commit: () => Promise<void> | void) => {
  swapQueue = swapQueue.then(async () => {
    swapping.value = true;
    emitMirror();
    await delay(160);
    await commit();
    swapping.value = false;
    emitMirror();
  });
};

const applyBubblePlacement = async (animate: boolean) => {
  const side = bubbleSide.value;
  const align = bubbleAlign.value;
  if (side === mirroredSide.value && align === mirroredAlign.value) return;
  const commit = async () => {
    await pushBubbleSide(side, align);
    mirroredSide.value = side;
    mirroredAlign.value = align;
  };
  if (!animate) {
    await commit();
    emitMirror();
    return;
  }
  runSwap(commit);
};

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

  unlisteners.push(
    await appWindow.listen(PET_BUBBLE_REQUEST, emitMirror),
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
    await appWindow.listen<{ side: string }>(PET_BUBBLE_SIDE_EVENT, (event) => {
      const side = event.payload?.side;
      if (
        side === "above" ||
        side === "below" ||
        side === "left" ||
        side === "right" ||
        side === "auto"
      ) {
        settingsStore.pet.bubbleSide = side;
        void syncBubblePlacement().then(() => applyBubblePlacement(true));
      }
    }),
    // 位置当场更新：气泡内容的内缩要逐帧跟随，等防抖那 120ms 会一顿一顿的
    await appWindow.onMoved(({ payload }) => {
      if (typeof payload?.x === "number") petLeftCss.value = payload.x / petDpr;
      if (typeof payload?.y === "number") petTopCss.value = payload.y / petDpr;
      schedulePlacementSync();
    }),
    await appWindow.listen<{ x: number; y: number }>("pet:cursor", (event) => {
      const { x, y } = event.payload;
      setShowChatInput(x >= 0 && y >= 0 && x <= window.innerWidth && y <= window.innerHeight);
    }),
    // 气泡窗回报「第 N 句已完整显示」
    await appWindow.listen<LineDrainedPayload>(PET_LINE_DRAINED, (event) => {
      onLineDrained(Number(event.payload?.lineId));
    }),
  );

  await applyWindowLayout();

  await syncBubblePlacement();
  await applyBubblePlacement(false);
  emitMirror();

  hitTestInterval = window.setInterval(reportSolidRegions, 100);
});

onUnmounted(() => {
  document.body.style.backgroundColor = "";
  document.documentElement.style.backgroundColor = "";
  unlisteners.forEach((unlisten) => unlisten());
  if (hitTestInterval !== undefined) window.clearInterval(hitTestInterval);
  if (placementTimer !== undefined) window.clearTimeout(placementTimer);
});

watch(scale, () => {
  void applyWindowLayout();
  schedulePlacementSync();
});

watch(
  () => [
    uiStore.showCharacterLine,
    uiStore.showCharacterEmotion,
    uiStore.currentAvatarAudio,
    gameStore.currentStatus,
    uiStore.notification.isVisible,
    uiStore.notification.message,
    scale.value,
    bubbleAlignInset.value,
  ],
  emitMirror,
);

const setShowChatInput = (insideWindow: boolean) => {
  showChatInput.value = insideWindow || (ChatInputRef.value?.isTyping() ?? false);
};

const handleMouseEnter = () => setShowChatInput(true);
const handleMouseLeave = () => setShowChatInput(false);

const requestFinishTyping = () => {
  void appWindow.emitTo("pet_bubble", PET_FINISH_TYPING_EVENT);
};

const { continueDialog } = useDialogAdvance({
  isTyping: lineTyping,
  finishTyping: requestFinishTyping,
});

const {
  typingFinished,
  onAudioStarted: handleAudioStarted,
  onAudioFinished: handleAudioFinished,
  manualTriggerContinue,
  cancelAdvance,
  scheduleAdvance,
  toggleAutoMode: handleSwitchAutoMode,
} = useAutoAdvance({
  // 打字机在气泡窗，没有组件句柄：用本地派生的 isTyping + 共用状态机拼一个
  dialog: () => ({ isTyping: lineTyping.value, continueDialog }),
  mergeEnabled: false,
});

/**
 * 气泡窗回报「第 N 句已完整显示」。
 *
 * 只认当前这句的号，过期或超前的回报一律丢弃；同一句重复回报（重新显示、清屏补发）
 * 不再重排延迟，否则延时会被无限续期。
 */
const onLineDrained = (drainedId: number) => {
  if (!Number.isFinite(drainedId) || drainedId !== currentLineId.value) return;
  if (drainedLineId.value === drainedId) return;
  drainedLineId.value = drainedId;
  typingFinished.value = true;
  scheduleAdvance();
};

// 先取消待调度，再走共用状态机（打字中先补全文字、不推进）
const handleAvatarClick = () => {
  manualTriggerContinue();
  continueDialog(true);
};

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

const handleExitPetMode = async () => {
  try {
    const settingsWindow = await WebviewWindow.getByLabel("settings");
    if (settingsWindow) await settingsWindow.close();
  } catch {
    return;
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
