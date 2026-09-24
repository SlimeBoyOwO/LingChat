<template>
  <div
    id="pet-app"
    :style="appStyleVars"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
    class="relative flex h-(--app-height) w-(--app-width) flex-col items-center justify-start overflow-hidden bg-transparent transition-none select-none"
  >
    <!-- 装饰带（气泡/通知）：高度完全随内容（无预留）→ 顶部永远没有透明空间：
         默认在宠物上方（气泡吸顶，宠物被往下让位）；设置=下方时夹在宠物与输入框之间（气泡贴宠物下沿） -->
    <div
      ref="decorBand"
      class="flex w-full shrink-0 flex-col justify-end bg-transparent transition-none"
      :style="{ order: bubbleBelow ? 1 : 0 }"
    >
      <PetNotification />
      <div class="flex items-end justify-center" :class="{ 'mb-1': bubbleVisible }">
        <DialogueBox ref="gameDialogRef" @player-continued="manualTriggerContinue" />
      </div>
    </div>

    <!-- Avatar 区域 -->
    <DragArea :isDragging="isDragging">
      <div
        ref="avatarContainer"
        class="flex shrink-0 items-center justify-center bg-transparent transition-all duration-100"
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

    <!-- ChatInput 区域（始终贴住上方元素：默认在宠物正下方，设置=下方时在气泡带之下） -->
    <div
      ref="chatContainer"
      class="flex w-full shrink-0 items-start justify-center bg-transparent transition-none"
      :style="{ height: 'var(--chat-h)', order: bubbleBelow ? 2 : 0 }"
    >
      <ChatInput ref="ChatInputRef" :visible="showChatInput" />
    </div>

    <!-- 余量吸收带：只在“下方”模式接管气泡带腾出的空间，保证窗口总高恒定（不上报 solid 区域） -->
    <div class="w-full flex-1" :style="{ order: bubbleBelow ? 3 : 0 }"></div>
  </div>
</template>

<script setup lang="ts">
import { useGameStore } from "@/stores/modules/game";
import { useSettingsStore, type BubbleSide } from "@/stores/modules/settings";
import { useUIStore } from "@/stores/modules/ui/ui";
import { invoke } from "@tauri-apps/api/core";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { currentMonitor, getCurrentWindow } from "@tauri-apps/api/window";
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { useFileDrop } from "../pet/useFileDrop";
import { useAutoAdvance } from "@/composables/chat/useAutoAdvance";

import ChatInput from "../pet/ChatInput.vue";
import DialogueBox from "../pet/DialogueBox.vue";
import DragArea from "../pet/DragArea.vue";
import GameRolesStage from "../pet/GameRolesStage.vue";
import PetNotification from "../pet/PetNotification.vue";
import { AVATAR_BAND_BASE, CHAT_BASE_H, DIALOG_MAX_BASE, PET_WIDTH_BASE } from "../pet/constants";

const { t } = useI18n();
const router = useRouter();
const gameStore = useGameStore();
const settingsStore = useSettingsStore();
const uiStore = useUIStore();

const showChatInput = ref(false);
const { isDragging, hasFile } = useFileDrop();

const avatarContainer = ref<HTMLElement | null>(null);
const chatContainer = ref<HTMLElement | null>(null);
const decorBand = ref<HTMLElement | null>(null);
const gameDialogRef = ref<InstanceType<typeof DialogueBox> | null>(null);
const ChatInputRef = ref<InstanceType<typeof ChatInput> | null>(null);

// 气泡/通知位置（用户设置）：above = 宠物上方，below = 宠物与输入框之间，auto = 按宠物在屏幕中的位置自动选
const bubbleSide = computed(() => settingsStore.pet?.bubbleSide ?? "above");
const autoBubbleBelow = ref(false);
// 自动判据看宠物圆心落在工作区上半还是下半：与气泡布局本身无关，不会来回抖动
const bubbleBelow = computed(
  () => bubbleSide.value === "below" || (bubbleSide.value === "auto" && autoBubbleBelow.value),
);

let autoSideTimer: number | undefined;
const refreshAutoBubbleSide = async () => {
  if (bubbleSide.value !== "auto") return;
  try {
    const [pos, monitor] = await Promise.all([
      getCurrentWindow().outerPosition(),
      currentMonitor(),
    ]);
    if (!monitor) return;
    const { position, size } = monitor.workArea;
    // 宠物可见圆心（窗口顶边 + 头像带一半）落在工作区上半 → 气泡下置
    const petCenterY =
      pos.y + (AVATAR_BAND_BASE * (settingsStore.pet?.scale ?? 1) * monitor.scaleFactor) / 2;
    autoBubbleBelow.value = petCenterY < position.y + size.height / 2;
  } catch {
    // 拿不到显示器信息时保持上一次判定
  }
};

// 原生拖拽期间 onMoved 会高频触发，去抖后再算
const scheduleAutoBubbleSide = () => {
  if (autoSideTimer !== undefined) window.clearTimeout(autoSideTimer);
  autoSideTimer = window.setTimeout(() => void refreshAutoBubbleSide(), 150);
};

// —— 换位 / 推挤动效（FLIP 思路）：flex 的 order 与“内容撑高”都无法过渡 ——
// 换位：切换前记下位置，反向 transform 起手再弹性归位；气泡带同时淡入；
// 推挤：气泡/通知撑高装饰带时，用高度差反推被顶开元素的旧位置，同样弹性滑回。
const MOTION_DURATION = 420;
const MOTION_EASING = "cubic-bezier(0.34, 1.28, 0.4, 1)"; // 末端轻微回弹
let swapStartTops: [HTMLElement, number][] = [];

const swapElements = () =>
  [decorBand.value, avatarContainer.value, chatContainer.value].filter(
    (el): el is HTMLElement => el !== null,
  );

// 从“旧位置”（相对当前布局偏移 dy）弹性滑回；fade 用于气泡带换位时的浮现
const animateFrom = (el: HTMLElement, dy: number, fade = false) => {
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches || Math.abs(dy) < 1) return;
  // 上一段动效直接归位，避免两次位移叠加
  el.getAnimations().forEach((anim) => anim.finish());
  el.animate(
    [
      { transform: `translateY(${dy}px)`, opacity: fade ? 0.25 : 1 },
      { transform: "translateY(0)", opacity: 1 },
    ],
    { duration: MOTION_DURATION, easing: MOTION_EASING },
  );
};

const captureSwapStart = () => {
  swapStartTops = swapElements().map((el) => {
    el.getAnimations().forEach((anim) => anim.finish());
    return [el, el.getBoundingClientRect().top];
  });
};

const playSwap = () => {
  for (const [el, startTop] of swapStartTops) {
    animateFrom(el, startTop - el.getBoundingClientRect().top, el === decorBand.value);
  }
  swapStartTops = [];
};

// 气泡上/下切换（拖拽进出屏幕上半区、设置里改选项）时播放换位动效
watch(bubbleBelow, async () => {
  captureSwapStart();
  await nextTick();
  playSwap();
});

// 气泡/通知出现、消失、改高会立刻把下方内容顶开 → 观察装饰带高度，把被顶开的元素弹性推回
let bandHeight: number | null = null;
const bandObserver = new ResizeObserver(() => {
  const band = decorBand.value;
  if (!band) return;
  const rect = band.getBoundingClientRect();
  const dy = bandHeight === null ? 0 : rect.height - bandHeight;
  bandHeight = rect.height;
  if (!dy) return;
  for (const el of swapElements()) {
    // 带子自身是“原地长高”，不位移；只有排在它下方被顶开的元素才回弹
    if (el !== band && el.getBoundingClientRect().top > rect.top) animateFrom(el, -dy);
  }
});

// 气泡当前是否有内容（显隐与点击穿透上报共用）
const bubbleVisible = computed(
  () => gameStore.currentStatus === "responding" && gameStore.currentLine.trim() !== "",
);

const appStyleVars = computed(() => {
  const scale = settingsStore.pet?.scale || 1.0;
  return {
    "--pet-ui-scale": scale.toString(),
    "--app-width": `${Math.round(PET_WIDTH_BASE * scale)}px`,
    "--app-height": `${Math.round((AVATAR_BAND_BASE + CHAT_BASE_H + DIALOG_MAX_BASE) * scale)}px`,
    "--avatar-size": `${Math.round(AVATAR_BAND_BASE * scale)}px`,
    "--chat-h": `${Math.round(CHAT_BASE_H * scale)}px`,
    "--dialog-h": `${Math.round(DIALOG_MAX_BASE * scale)}px`,
  };
});

const applyWindowLayout = async () => {
  try {
    const scale = settingsStore.pet?.scale || 1.0;
    await invoke("set_pet_mode", { enable: true, scale });
  } catch (error) {
    console.error("调整窗口布局失败:", error);
  }
};

let hitTestInterval: number | undefined;
let scaleUnlisten: (() => void) | null = null;
let effectUnlisten: (() => void) | null = null;
let volumeUnlisten: (() => void) | null = null;
let live2dFpsUnlisten: (() => void) | null = null;
let dialogHistoryUnlisten: (() => void) | null = null;
let cursorUnlisten: (() => void) | null = null;
let bubbleSideUnlisten: (() => void) | null = null;
let movedUnlisten: (() => void) | null = null;

onMounted(async () => {
  const appWindow = getCurrentWindow();

  scaleUnlisten = await appWindow.listen<{ scale: number }>("pet-scale-changed", (event) => {
    const scale = Number(event.payload?.scale);
    if (!Number.isNaN(scale)) {
      settingsStore.pet.scale = scale;
      void applyWindowLayout();
    }
  });

  effectUnlisten = await appWindow.listen<{ effect: string }>(
    "background-effect-changed",
    (event) => {
      const effect = event.payload?.effect;
      if (effect) {
        uiStore.setBackgroundEffect(effect);
      }
    },
  );

  volumeUnlisten = await appWindow.listen<{ volume: number }>("pet-volume-changed", (event) => {
    const volume = Number(event.payload?.volume);
    if (!Number.isNaN(volume)) {
      settingsStore.updateAudio({ characterVolume: volume });
    }
  });

  // 设置窗口修改 Live2D 帧率后同步到本窗口 store；GameRolesStage 响应式读取即热生效
  live2dFpsUnlisten = await appWindow.listen<{ fps: number }>("pet-live2d-fps-changed", (event) => {
    const fps = Number(event.payload?.fps);
    if (!Number.isNaN(fps)) {
      settingsStore.setPetLive2dFps(fps);
    }
  });

  // 响应设置窗口的初始历史数据请求
  dialogHistoryUnlisten = await appWindow.listen("request-dialog-history", () => {
    appWindow.emit("dialog-history-changed", {
      dialogHistory: JSON.parse(JSON.stringify(gameStore.dialogHistory)),
    });
  });

  // 输入框显隐兜底：光标是否仍在桌宠窗口内。不能只靠 #pet-app 的
  // mouseenter/mouseleave —— 光标离开 solid 区域后窗口会自动开启点击穿透
  // （见 src-tauri/src/api/pet.rs 的 spawn_hit_test_poll），webview 从此收不到
  // 鼠标事件，mouseleave 可能永远不来、输入框再也隐藏不掉。pet:cursor 是 Rust 侧
  // 全局轮询广播（每 50ms，窗口内逻辑坐标，与 DOM 同坐标系），可兜住这种情况。
  cursorUnlisten = await appWindow.listen<{ x: number; y: number }>("pet:cursor", (event) => {
    const { x, y } = event.payload;
    setShowChatInput(x >= 0 && y >= 0 && x <= window.innerWidth && y <= window.innerHeight);
  });

  // 设置窗口改了气泡位置：即时换位（纯 CSS 换 order，不动窗口尺寸，不会闪）
  bubbleSideUnlisten = await appWindow.listen<{ side: BubbleSide }>(
    "pet-bubble-side-changed",
    (event) => {
      if (event.payload?.side) settingsStore.pet.bubbleSide = event.payload.side;
    },
  );

  // 自动模式：窗口移动（原生拖拽、换屏）后重算气泡在上还是在下
  movedUnlisten = await appWindow.onMoved(scheduleAutoBubbleSide);
  await refreshAutoBubbleSide();

  // 气泡/通知撑高装饰带时，把被顶开的内容弹性推回（见 bandObserver）
  if (decorBand.value) bandObserver.observe(decorBand.value);

  // 设置透明背景的 body 属性样式（额外防护）
  document.body.style.backgroundColor = "transparent";
  document.documentElement.style.backgroundColor = "transparent";

  // 1. 初始化窗口为桌宠尺寸
  await applyWindowLayout();

  // 2. 启动 100ms 一次的 solid bounds 测试
  // 挂机时各区域 rect 恒定不变，先做内容比对、有变化才走 IPC，避免 10Hz 空转唤醒后端
  let lastRectsKey = "";
  hitTestInterval = window.setInterval(() => {
    const rects = [];

    // 如果对话气泡正在显示，则加入 solid region（用气泡元素精确 rect，避免包住整个对话框）
    if (gameDialogRef.value?.bubbleRef && bubbleVisible.value) {
      const r = gameDialogRef.value.bubbleRef.getBoundingClientRect();
      if (r.height > 0) {
        rects.push({ x: r.x, y: r.y, width: r.width, height: r.height });
      }
    }

    // 头像圆环常驻 solid region 触发拖拽和交互
    if (avatarContainer.value) {
      const r = avatarContainer.value.getBoundingClientRect();
      rects.push({ x: r.x, y: r.y, width: r.width, height: r.height });
    }

    // 输入框显示时，加入 solid region
    if (chatContainer.value && showChatInput.value) {
      const r = chatContainer.value.getBoundingClientRect();
      // 输入框稍微拓宽，保证极小尺寸下的鼠标判定连贯性
      rects.push({
        x: r.x - 20,
        y: r.y - 20,
        width: r.width + 40,
        height: r.height + 40,
      });
    }

    const rectsKey = JSON.stringify(rects);
    if (rectsKey === lastRectsKey) return;
    lastRectsKey = rectsKey;
    invoke("update_solid_regions", { rects }).catch(() => {
      // 失败时清空缓存，让下一轮重试上报
      lastRectsKey = "";
    });
  }, 100);
});

watch(
  () => settingsStore.pet?.scale,
  () => {
    void applyWindowLayout();
  },
);

// 设置里切到/切出“自动”时立即重算一次
watch(bubbleSide, () => void refreshAutoBubbleSide());

// 监听 dialogHistory 变化，推送给设置窗口
watch(
  () => gameStore.dialogHistory.length,
  () => {
    const appWindow = getCurrentWindow();
    appWindow.emit("dialog-history-changed", {
      dialogHistory: JSON.parse(JSON.stringify(gameStore.dialogHistory)),
    });
  },
);

onUnmounted(() => {
  // 恢复默认背景色
  document.body.style.backgroundColor = "";
  document.documentElement.style.backgroundColor = "";

  if (scaleUnlisten) scaleUnlisten();
  if (effectUnlisten) effectUnlisten();
  if (volumeUnlisten) volumeUnlisten();
  if (live2dFpsUnlisten) live2dFpsUnlisten();
  if (dialogHistoryUnlisten) dialogHistoryUnlisten();
  if (cursorUnlisten) cursorUnlisten();
  if (bubbleSideUnlisten) bubbleSideUnlisten();
  if (movedUnlisten) movedUnlisten();
  if (autoSideTimer !== undefined) window.clearTimeout(autoSideTimer);
  bandObserver.disconnect();

  if (hitTestInterval !== undefined) {
    window.clearInterval(hitTestInterval);
  }
});

// 光标在桌宠窗口内就显示输入框，离开则隐藏；草稿非空（正在打字）时保持显示
const setShowChatInput = (insideWindow: boolean) => {
  showChatInput.value = insideWindow || (ChatInputRef.value?.isTyping() ?? false);
};

const handleMouseEnter = () => {
  setShowChatInput(true);
};

const handleMouseLeave = () => {
  setShowChatInput(false);
};

const handleAvatarClick = () => {
  // 走 continueDialog 而非直接 eventQueue.continue()：后者绕过了「打字中先补全文本
  // 再推进」的守卫，也跳过 player-continued/dialog-proceed 派发——打字中点头像会
  // 把正在打的字丢掉。continueDialog 在真的推进时会派发 player-continued，
  // 由 @player-continued 绑定的 manualTriggerContinue 取消待触发的自动推进定时器。
  gameDialogRef.value?.continueDialog(true);
};

const handleOpenSettings = async () => {
  try {
    const existing = await WebviewWindow.getByLabel("settings");
    if (existing) {
      await existing.setFocus();
      return;
    }

    const webview = new WebviewWindow("settings", {
      url: "/second",
      title: t("views.petMode.settingsWindowTitle"),
      width: 1200,
      height: 800,
      resizable: true,
      shadow: false,
      decorations: false,
      transparent: true,
      alwaysOnTop: false,
    });

    webview.once("tauri://created", () => {
      console.log("桌宠轻量设置窗口创建成功");
    });

    webview.once("tauri://error", (e) => {
      console.error("创建桌宠轻量设置窗口失败:", e);
    });
  } catch (error) {
    console.error("打开设置窗口时出错:", error);
  }
};

// 自动推进调度 —— 与主界面 MainChat 共用同一实现。
// 桌宠不参与台词合并，故 mergeEnabled 为 false。
const {
  onAudioStarted: handleAudioStarted,
  onAudioFinished: handleAudioFinished,
  manualTriggerContinue,
  toggleAutoMode: handleSwitchAutoMode,
} = useAutoAdvance({
  dialog: () => gameDialogRef.value,
  mergeEnabled: false,
});

const handleExitPetMode = async () => {
  // 关闭设置窗口（如果打开的话）
  try {
    const settingsWindow = await WebviewWindow.getByLabel("settings");
    if (settingsWindow) {
      await settingsWindow.close();
    }
  } catch {
    // 窗口不存在，忽略
  }

  // 退出时清除 solid region，防止残留
  await invoke("update_solid_regions", { rects: [] });
  // 1. 关闭桌宠窗口特性，恢复 1500x800 的正常主窗口
  await invoke("set_pet_mode", { enable: false });
  // 2. 路由导航回聊天主页面
  router.push("/chat");
};
</script>

<style scoped>
#pet-app {
  position: relative;
  width: 100vw;
  height: 100dvh;
  overflow: hidden;
}
</style>
