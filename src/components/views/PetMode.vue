<template>
  <div
    id="pet-app"
    :style="appStyleVars"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
    class="relative flex h-(--app-height) w-(--app-width) flex-col items-center justify-start overflow-hidden bg-transparent transition-none select-none"
  >
    <!-- 位移占位：气泡在下置模式时用它与窗口上移量等高的空间顶住头像，
         使上下切换的那一瞬头像不会跳（上置模式由装饰带高度承担同样职责） -->
    <div
      class="w-full shrink-0"
      :style="{ height: bandAbove ? '0px' : `${shiftCss}px`, order: -1 }"
    ></div>

    <!-- 装饰带（气泡/通知）：上置时高度 = 当前位移（与窗口上移量同步推进，气泡从宠物头顶平滑升起）；
         空闲/下置时随内容 → 顶部永远没有透明空间，宠物可以贴屏幕顶 -->
    <div
      ref="decorBand"
      class="flex w-full shrink-0 flex-col justify-end bg-transparent transition-none"
      :style="{ order: bubbleBelow ? 1 : 0, height: bandAbove ? `${shiftCss}px` : 'auto' }"
    >
      <div class="flex w-full shrink-0 flex-col">
        <PetNotification />
        <div class="flex items-end justify-center" :class="{ 'mb-1': bubbleVisible }">
          <DialogueBox ref="gameDialogRef" @player-continued="manualTriggerContinue" />
        </div>
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
import { currentMonitor, getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";
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

// 气泡/通知位置（用户设置）：above = 宠物上方（不自动翻转，放不下时请手动改成下方）；below = 强制下置。
// 旧版本可能存过 auto，一律按 above 处理
const bubbleSide = computed<BubbleSide>(() =>
  settingsStore.pet?.bubbleSide === "below" ? "below" : "above",
);
// 气泡当前是否有内容（显隐与点击穿透上报共用）
const bubbleVisible = computed(
  () => gameStore.currentStatus === "responding" && gameStore.currentLine.trim() !== "",
);

// —— 窗口补偿：头像必须钉在屏幕上不动，而“上置”的气泡在窗口内只能挤在头像上方 ——
// 做法：装饰带高度与窗口上移量始终相等、同步逐帧推进 —— 头像屏幕位置恒等于“归位顶边”，
// 气泡只是在自己这块预留区里淡入淡出。位移归零（下置模式）时顶部没有任何预留空间。
// 逐帧推进是必需的：窗口位置由 OS 合成、内容由 webview 合成，一次跳完整段必然会有
// 一帧错位（表现就是“上下抽动”）；分帧后单帧错位 ≤ 一步，肉眼不可见。
const SHIFT_STEP_PX = 12; // 每帧最大位移：越小越平滑，越大越快
const petScale = computed(() => settingsStore.pet?.scale || 1);
const layoutReady = ref(false);
const restingTopCss = ref(0); // 归位后的窗口顶边（= 头像在屏幕上的顶边）
const restingLeftCss = ref(0);
const workTopCss = ref(0);
const dprRef = ref(window.devicePixelRatio || 1);
const shiftCss = ref(0); // 当前位移：同时决定“装饰带高度”与“窗口上移量”

// 上置需要预留的高度：整块气泡预算，且与“有没有内容”无关（常备）——
// 正因如此，气泡出现/消失不会引起任何窗口移动
const reserveCss = computed(() => DIALOG_MAX_BASE * petScale.value);
// above = 永远上置，不再自动翻到下置（贴顶时上方放不下，请用户自行改成“下方”）；
// 上移量按工作区顶边裁剪，避免把窗口推到屏幕外
const bubbleBelow = computed(() => bubbleSide.value === "below");
const bandAbove = computed(() => !bubbleBelow.value);
// 位移目标：只取决于放置模式，与气泡/通知内容高度无关 —— 上置时窗口“常备”抬升整块预算，
// 气泡只是在自己预留区里淡入淡出。内容变化时窗口与布局都不动 → 不会上下抽动。
const targetShiftCss = computed(() => {
  if (!bandAbove.value) return 0;
  if (!layoutReady.value) return 0; // 还没拿到真实窗口位置，先不抬
  const room = Math.max(0, restingTopCss.value - workTopCss.value);
  return Math.min(reserveCss.value, room);
});

let shiftRaf: number | undefined;

const stepWindowShift = () => {
  shiftRaf = undefined;
  if (!layoutReady.value) return; // 还没拿到真实窗口位置，先不动
  const target = targetShiftCss.value;
  const delta = target - shiftCss.value;
  // 亚像素直接抹平：浮点残差会让循环永不收敛（每帧都在移窗口 → 闪动）
  if (Math.abs(delta) < 0.5) {
    if (delta) shiftCss.value = target;
    reportSolidRegions(); // 收敛后再报一次：此时 DOM 已经稳定
    return;
  }
  // 步长自适应：差值大时尽快跟上（气泡不被久切），快到位时收小；
  // 必须再按 |delta| 封顶 —— 步长大于差值就会来回震荡、永不收敛
  const step = Math.min(SHIFT_STEP_PX, Math.max(2, Math.abs(delta) * 0.4), Math.abs(delta));
  shiftCss.value += Math.sign(delta) * step;
  void getCurrentWindow()
    .setPosition(
      new PhysicalPosition(
        Math.round(restingLeftCss.value * dprRef.value),
        Math.round((restingTopCss.value - shiftCss.value) * dprRef.value),
      ),
    )
    .catch(() => {
      // 移动失败：下一次同步或拖拽会重新对齐
    });
  reportSolidRegions(); // 视口坐标变了，立即重报，避免被误判成“光标不在桌宠上”
  scheduleWindowShift();
};

const scheduleWindowShift = () => {
  if (shiftRaf === undefined) shiftRaf = window.requestAnimationFrame(stepWindowShift);
};

// 读取窗口/显示器信息：刷新归位顶边与工作区，并重判“上方是否放得下”
const syncPetPlacement = async () => {
  try {
    const [pos, monitor] = await Promise.all([
      getCurrentWindow().outerPosition(),
      currentMonitor(),
    ]);
    if (!monitor) return;
    dprRef.value = monitor.scaleFactor || window.devicePixelRatio || 1;
    // 当前窗口顶边 + 已上移量 = 归位后的顶边（用户拖动窗口后也靠这一步重新对齐）。
    // 位移进行中不能重算：此时窗口位置是半途值，会把基准越算越低 → 宠物缓慢下沉
    if (shiftRaf === undefined) {
      restingTopCss.value = (pos.y + shiftCss.value * dprRef.value) / dprRef.value;
    }
    restingLeftCss.value = pos.x / dprRef.value;
    workTopCss.value = monitor.workArea.position.y / dprRef.value;
    layoutReady.value = true;
    scheduleWindowShift();
  } catch {
    // 拿不到窗口/显示器信息时保持上一次判定
  }
};

// 原生拖拽期间 onMoved 会高频触发，去抖后再算
let placementTimer: number | undefined;
const schedulePlacementSync = () => {
  if (placementTimer !== undefined) window.clearTimeout(placementTimer);
  placementTimer = window.setTimeout(() => void syncPetPlacement(), 150);
};

// 窗口被移动（原生拖拽/换屏）：水平方向只可能是用户拖动（补偿只改 y），必须立即跟随，
// 否则下一次补偿移动会用过期的 x 把窗口拉回去 → 拖不动
const onWindowMoved = (event: { payload: PhysicalPosition }) => {
  restingLeftCss.value = event.payload.x / dprRef.value;
  schedulePlacementSync();
};

// —— 换位动效：只对装饰带做一次屏幕坐标下的 FLIP ——
// 安全性：装饰带的屏幕位置只由它自己的 transform 决定（布局上移量与窗口上移量始终是同一个
// shiftCss、等量抵消），因此这段动画与逐帧位移、命中测试、头像/输入框位置完全解耦。
const SWAP_DURATION = 260;
const bandVisualTop = () => (decorBand.value?.getBoundingClientRect().top ?? 0) - shiftCss.value;

watch(bubbleBelow, async () => {
  const band = decorBand.value;
  if (!band || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  band.getAnimations().forEach((anim) => anim.finish()); // 连续切换时先归位，避免叠加
  const from = bandVisualTop(); // 换位前的屏幕位置
  await nextTick();
  const dy = from - bandVisualTop();
  if (Math.abs(dy) < 1) return;
  band.animate([{ transform: `translateY(${dy}px)` }, { transform: "translateY(0)" }], {
    duration: SWAP_DURATION,
    easing: "cubic-bezier(0.34, 1.28, 0.4, 1)", // 末端轻微回弹
  });
});

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

// solid 区域上报（视口坐标，Rust 侧按当前窗口位置换算成屏幕坐标）：
// 挂机时各区域 rect 恒定不变，先做内容比对、有变化才走 IPC，避免 10Hz 空转唤醒后端。
// 窗口补偿移动窗口后会立即再报一次（见 stepWindowShift），否则旧坐标会短暂误判。
let lastRectsKey = "";
const reportSolidRegions = () => {
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
};
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
  movedUnlisten = await appWindow.onMoved(onWindowMoved);
  await syncPetPlacement();

  // 设置透明背景的 body 属性样式（额外防护）
  document.body.style.backgroundColor = "transparent";
  document.documentElement.style.backgroundColor = "transparent";

  // 1. 初始化窗口为桌宠尺寸
  await applyWindowLayout();

  // 2. 启动 100ms 一次的 solid bounds 测试
  hitTestInterval = window.setInterval(reportSolidRegions, 100);
});

watch(
  () => settingsStore.pet?.scale,
  () => {
    void applyWindowLayout();
  },
);

// 设置里切到/切出“自动”时立即重算一次
watch(bubbleSide, () => void syncPetPlacement());

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
  if (placementTimer !== undefined) window.clearTimeout(placementTimer);
  if (shiftRaf !== undefined) window.cancelAnimationFrame(shiftRaf);

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
