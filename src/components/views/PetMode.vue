<template>
  <div
    id="pet-app"
    v-show="!returningToApp"
    :style="appStyleVars"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
    class="relative flex h-(--app-height) w-(--app-width) flex-col items-center justify-start overflow-hidden bg-transparent transition-none select-none"
  >
    <!-- 悬浮窗展开态的返回按钮：收回悬浮窗并切回聊天页。

         位置必须在逻辑画布**内部**（top-1 / right-1，而不是 -top-1 / -right-1）：
         早先挂在头像右上角用负偏移，整体等比缩放后会被 #pet-app 的
         overflow-hidden 裁掉一半，真机上根本点不到。

         z 值给到 100：气泡带、头像里的 Live2D 画布都是同层的定位元素，
         给低了会被压在下面看不见。

         只在展开态出现——收起态只有头像、没有放按钮的地方，而展开本来就靠
         点头像，退出需要一个明确、看得见的入口。 -->
    <button
      v-if="floatingWindowMode && petExpanded"
      type="button"
      aria-label="返回"
      title="返回"
      class="absolute top-1 right-1 z-[100] flex h-7 w-7 items-center justify-center rounded-full border border-white/25 bg-neutral-950/85 text-white/95 shadow-lg backdrop-blur-xl active:scale-95"
      @click.stop="handleExitPetMode"
    >
      <ArrowLeft :size="15" />
    </button>

    <!-- 装饰带（气泡/通知）：高度完全随内容（无预留）→ 顶部永远没有透明空间：
         默认在宠物上方（气泡吸顶，宠物被往下让位）；设置=下方时夹在宠物与输入框之间（气泡贴宠物下沿）

         悬浮窗收起态不渲染：窗口只有 1/6 屏宽，整体缩放系数约 0.25，
         气泡里的字会小到看不清，没有可读空间。
         悬浮窗展开态则**必须**排到头像之后（order=1）：气泡撑高窗口时
         头像不动、只有输入框下移，视觉上气泡像是从宠物下方长出来。 -->
    <div
      v-show="!(floatingWindowMode && !petExpanded)"
      ref="decorBand"
      class="flex w-full shrink-0 flex-col justify-end bg-transparent transition-none"
      :style="{ order: floatingWindowMode || bubbleBelow ? 1 : 0 }"
    >
      <!-- 悬浮窗里不显示通知条：窗口太小，通知会挤占头像 -->
      <PetNotification v-if="!floatingWindowMode" />
      <div class="flex items-end justify-center" :class="{ 'mb-1': bubbleVisible }">
        <DialogueBox ref="gameDialogRef" @player-continued="manualTriggerContinue" />
      </div>
    </div>

    <!-- Avatar 区域 -->
    <DragArea :isDragging="isDragging">
      <div
        ref="avatarContainer"
        class="relative flex shrink-0 items-center justify-center bg-transparent transition-all duration-100"
        :style="
          floatingWindowMode
            ? undefined
            : { width: 'var(--avatar-size)', height: 'var(--avatar-size)' }
        "
      >
        <GameRolesStage
          @avatar-click="handleAvatarClick"
          @open-settings="handleOpenSettings"
          @switch-auto-mode="handleSwitchAutoMode"
          @exit-pet-mode="handleExitPetMode"
          @audio-ended="handleAudioFinished"
          @audio-started="handleAudioStarted"
        />

        <!-- 悬浮窗里不再放「收起 / 关闭」按钮。
             原先那两个圆形按钮挂在头像右上角（-top-1 -right-1），在
             整体缩放的悬浮窗里会被 #pet-app 的 overflow-hidden 裁掉一半，
             实测点不到。手机上的手势约定改为：
             点头像 = 展开/收起切换，双击头像 = 收回 App。
             少两个按钮同时也少一次「按钮在不在窗口内」的布局风险。 -->
      </div>
    </DragArea>

    <!-- ChatInput 区域（始终贴住上方元素：默认在宠物正下方，设置=下方时在气泡带之下）

         悬浮窗收起态不渲染：此时只有头像，输入框在展开后才出现。 -->
    <div
      v-show="!(floatingWindowMode && !petExpanded)"
      ref="chatContainer"
      class="flex w-full shrink-0 items-start justify-center bg-transparent transition-none"
      :style="{ height: 'var(--chat-h)', order: floatingWindowMode || bubbleBelow ? 2 : 0 }"
    >
      <ChatInput ref="ChatInputRef" :visible="showChatInput" />
    </div>

    <!-- 余量吸收带：只在“下方”模式接管气泡带腾出的空间，保证窗口总高恒定（不上报 solid 区域）

         悬浮窗里必须排在最后（order=3）：它带 flex-1，若 order 仍是 0
         会插到头像与气泡之间，把气泡挤到窗口底部。 -->
    <div class="w-full flex-1" :style="{ order: floatingWindowMode || bubbleBelow ? 3 : 0 }"></div>
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
import {
  getFloatingPetStatus,
  hideFloatingPet,
  isInFloatingWindow,
  markFloatingWindowMode,
  onFloatingWindowModeChange,
  onPetExpandedChange,
  onPetMetrics,
  resizeFloatingPet,
  setFloatingPetExpanded,
} from "@/api/services/floating-pet";

import ChatInput from "../pet/ChatInput.vue";
import DialogueBox from "../pet/DialogueBox.vue";
import DragArea from "../pet/DragArea.vue";
import GameRolesStage from "../pet/GameRolesStage.vue";
import PetNotification from "../pet/PetNotification.vue";
import { ArrowLeft } from "lucide-vue-next";
import { isAndroid } from "@/utils/platform";
import { AVATAR_BAND_BASE, CHAT_BASE_H, DIALOG_MAX_BASE, PET_WIDTH_BASE } from "../pet/constants";

const { t } = useI18n();
const router = useRouter();
const gameStore = useGameStore();
const settingsStore = useSettingsStore();
const uiStore = useUIStore();

const showChatInput = ref(false);
const { isDragging, hasFile } = useFileDrop();

/**
 * 是否运行在 Android 悬浮窗里。
 *
 * 悬浮窗里放的是**主 WebView 本身**（原生搬运视图，不是新建实例），
 * 所以 `getCurrentWindow()` 和 `invoke()` 在这里都**是正常的**——
 * 早期那套「用 try/catch getCurrentWindow 探测」的判据已经失效。
 *
 * 现在由原生在搬移完成后派发 `pet-detached` 事件告知，见
 * {@link onFloatingWindowModeChange}。
 */
const floatingWindowMode = ref(isInFloatingWindow());

/**
 * 已从悬浮窗收回、正在等路由切回 `/chat`。
 *
 * 收回的那一瞬间页面还停在 `/pet`：`floatingWindowMode` 已经是 false，
 * 于是走桌面分支按 240×480 渲染——在整屏 Activity 里就是**左上角一小块**，
 * 看起来和「没收回去」一模一样。`router.push("/chat")` 落地通常只要几十
 * 毫秒，但慢机器上足够被看见。这里先把桌宠页藏起来。
 */
const returningToApp = ref(false);
let returningTimer: number | undefined;

/**
 * 悬浮窗的收起/展开态。
 *
 * 收起 = 只显示头像（窗口约 1/6 屏宽）；展开 = 头像 + 输入框（约 2/5 屏宽）。
 *
 * 手机上没有鼠标悬停，因此展开由「点击头像」触发，这与桌面端的
 * `mouseenter/mouseleave` 是本质差异。窗口尺寸同步由原生改，
 * 页面通过 `pet-expanded-changed` 事件得知结果。
 */
const petExpanded = ref(false);

const avatarContainer = ref<HTMLElement | null>(null);
const chatContainer = ref<HTMLElement | null>(null);
const decorBand = ref<HTMLElement | null>(null);
const gameDialogRef = ref<InstanceType<typeof DialogueBox> | null>(null);
const ChatInputRef = ref<InstanceType<typeof ChatInput> | null>(null);

// ─── 悬浮窗的「逻辑画布 + 整体缩放」模型 ───────────────────────────
//
// 悬浮窗里的页面**不做响应式布局**：始终按桌面端那套 240dp 宽的布局排版
// （下称逻辑画布），再整体 `transform: scale(窗口宽度 / 240)` 缩放到窗口。
//
// 之前是让布局跟着窗口宽度走，结果是三件事同时坏掉：
//   1. 展开态窗口 2.75 倍宽，但头像仍按收起态宽度渲染 → 窗口里一大片透明区
//      （而 Android 悬浮窗没有逐像素穿透，那片区域还会吃掉触摸）
//   2. 输入框、按钮、字号不会跟着缩 → 小窗里挤成一团、点不到
//   3. 布局在两种形态下走不同分支 → 只有一套分支被真机验证过
//
// 换成整体缩放后布局只有一套（与桌面端完全一致），内容恰好铺满逻辑画布，
// 于是窗口里没有透明区、所有控件等比可点。
//
// 代价：文字绝对大小与窗口宽度成正比，所以展开态不能太窄——见
// FloatingPetPlugin.kt 的 EXPANDED_WIDTH_RATIO（取 0.6 屏宽，缩放系数约 0.9）。
const FLOATING_LOGICAL_WIDTH = PET_WIDTH_BASE;

/** 逻辑画布 → 实际窗口的缩放系数。仅悬浮窗模式有意义。 */
const floatingFit = ref(1);

/**
 * 逻辑画布的内容高度（未缩放）。
 *
 * 由 {@link reportFloatingHeight} 从实际 DOM 量出来回填——气泡是流式
 * 输出的，高度随时在变，写死常量必然算错。初值取收起态的头像带高度。
 */
const floatingContentHeight = ref(AVATAR_BAND_BASE);

/** 画布高度：至少容纳当前形态的固定部分，再多容纳气泡。 */
const floatingCanvasHeight = computed(() => {
  const base = petExpanded.value ? AVATAR_BAND_BASE + CHAT_BASE_H : AVATAR_BAND_BASE;
  return Math.max(base, floatingContentHeight.value);
});

/**
 * 是否已经收到过原生推来的权威几何。
 *
 * 收到之后就**不再**从 `window.innerWidth` 自算：那个值在原生刚改完
 * 窗口尺寸时是滞后的，自算反而会把正确的系数覆盖成错的。
 */
let metricsReceived = false;

/** 兜底：还没收到 pet-metrics 时，先从视口宽度自算一个系数。 */
const syncFloatingFit = () => {
  if (!isInFloatingWindow()) return;
  if (metricsReceived) return;
  const k = window.innerWidth / FLOATING_LOGICAL_WIDTH;
  if (k > 0) floatingFit.value = k;
};

/** 悬浮窗尺寸变化（展开/收起、气泡撑高、原生改尺寸）后重算并回报。 */
const onFloatingResize = () => {
  syncFloatingFit();
  reportFloatingHeight();
};

/**
 * 把内容高度上报给原生，让窗口恰好裹住内容。
 *
 * 原生只知道宽度（按屏幕比例算），高度得由页面说了算——气泡出现时
 * 内容会变高，窗口必须跟着长，否则气泡被裁掉、用户以为「消息发不出去」。
 *
 * 高度只依赖内容（头像带 + 输入带 + 气泡带），**不依赖窗口高度**，
 * 因此这里不会和原生形成「改高度 → 重排 → 再改高度」的来回震荡。
 * `lastReportedHeight` 再去掉重复上报。
 */
let lastReportedHeight = -1;
const reportFloatingHeight = () => {
  if (!floatingWindowMode.value) return;
  const k = floatingFit.value;
  if (!(k > 0)) return;
  const base = petExpanded.value ? AVATAR_BAND_BASE + CHAT_BASE_H : AVATAR_BAND_BASE;
  // offsetHeight 是布局尺寸（未乘 transform），正是逻辑画布里的高度
  const band = decorBand.value?.offsetHeight ?? 0;
  const logical = base + band;
  // 先让画布长高再报尺寸：反过来的话，窗口先变大而画布还是旧的，
  // 中间那一帧气泡会把输入框顶出画布、被 overflow-hidden 裁掉。
  floatingContentHeight.value = logical;
  const height = Math.round(logical * k);
  if (height <= 0) return;
  // 容差 2px：原生改高度后视口可能抖动 1px，进而让算出的高度抖 1px。
  // 没有容差就会和原生来回改尺寸停不下来。
  if (lastReportedHeight > 0 && Math.abs(height - lastReportedHeight) <= 2) return;
  lastReportedHeight = height;
  // 宽度传 0 = 「只改高度」：宽度归原生独占。回传 window.innerWidth 会踩到
  // 视口滞后——原生刚改完尺寸时那个值还是旧的，等于把刚展开的窗口缩回去。
  void resizeFloatingPet(0, height).catch(() => {
    // 失败时清掉缓存，下一轮重试
    lastReportedHeight = -1;
  });
};

/**
 * 延迟回报内容高度（兜底）。
 *
 * 正常情况下 `pet-metrics` 一到就会回报，这里只是防止那一轮事件丢失
 * （例如原生改尺寸与页面改布局撞在一起）。去抖避免连续展开/收起时堆积。
 */
let heightReportTimer: number | undefined;
const scheduleHeightReport = (delay = 200) => {
  if (heightReportTimer !== undefined) window.clearTimeout(heightReportTimer);
  heightReportTimer = window.setTimeout(() => {
    heightReportTimer = undefined;
    reportFloatingHeight();
  }, delay);
};

// ─── 主动查询原生状态（页面 → 原生） ─────────────────────────────
//
// 原生往页面推事件只能用 `evaluateJavascript`，而这条路在搬运/收回前后
// **并不可靠**：WebView 刚被挂上、宿主 Activity 还在后台、视口尚未就绪……
// 实测「即使在前台收回，页面也仍然停在悬浮窗布局」。
//
// 反过来，**页面 → 原生**的 Tauri IPC 是稳的——用户点 ✕、发消息都走它。
// 因此把「我现在还在不在悬浮窗里」和「窗口多大」都改成主动查询：
// 每 500ms 问一次，漏了哪一次都会在下一轮自愈。

/** 几何轮询间隔（毫秒）。 */
const METRICS_POLL_MS = 500;
let metricsTimer: number | undefined;

/**
 * 是否曾经观察到「确实在悬浮窗里」。
 *
 * 进入流程是「先切 /pet 路由 → 再调 show 搬移」，页面挂载时原生还没搬，
 * 第一次查询必然是 `detached: false`。没有这个标记就会把「还没搬进去」
 * 误判成「已经收回来了」，直接把用户弹回聊天页。
 */
let sawDetached = false;

/**
 * 已回到 Activity：重置形态、切回聊天页。
 *
 * 事件（`pet-attached`）与轮询（`detached` 变 false）两条路都走它，
 * 且**幂等**——重复调用只会重复 push 同一个路由，vue-router 会忽略。
 */
const handleReturnedToApp = () => {
  if (!floatingWindowMode.value) return;
  floatingWindowMode.value = false;
  markFloatingWindowMode(false);
  stopMetricsPolling();
  petExpanded.value = false;
  showChatInput.value = false;
  // 页面不再缩放：不归位的话 --pet-fit 还留着悬浮窗里的系数（约 0.25），
  // 整页会被缩成左上角一小块。
  floatingFit.value = 1;
  lastReportedHeight = -1;
  // 导航落地前先藏起桌宠页，避免它按桌面尺寸（240×480）在整屏 Activity
  // 左上角闪一下。1.5 秒兜底：万一导航没落地，也不能让页面一直空着。
  returningToApp.value = true;
  if (returningTimer !== undefined) window.clearTimeout(returningTimer);
  returningTimer = window.setTimeout(() => {
    returningTimer = undefined;
    returningToApp.value = false;
  }, 1500);
  // 临时诊断：这一帧的视口尺寸就是「只有左上一角」的关键证据
  showViewportDiagnostic("returned");
  void router.push("/chat");
};

const stopMetricsPolling = () => {
  if (metricsTimer === undefined) return;
  window.clearInterval(metricsTimer);
  metricsTimer = undefined;
};

/** 查询一次原生状态；查询失败保持现状，等下一轮。 */
const pollNativeState = async () => {
  if (!isInFloatingWindow()) return;
  try {
    const status = await getFloatingPetStatus();
    nativeWindowWidth.value = status.width;
    if (status.detached) {
      sawDetached = true;
      // 兜底自愈：万一进悬浮窗时的事件丢了、页面还停在桌面端布局
      // （见 enterFloatingLayout 的说明），这里按原生的权威答案切回来。
      if (!floatingWindowMode.value) enterFloatingLayout();
      if (status.scale > 0) {
        metricsReceived = true;
        floatingFit.value = status.scale;
      }
      reportFloatingHeight();
      showViewportDiagnostic("floating");
      return;
    }
    // 原生说 WebView 已经不在悬浮窗里了 → 按「已回到 App」处理。
    // 只有**见过** detached 才认，否则会误伤「刚挂载、还没搬进去」。
    if (sawDetached) handleReturnedToApp();
  } catch {
    // 插件不可用或瞬时失败，下一轮重试
  }
};

const startMetricsPolling = () => {
  if (metricsTimer !== undefined) return;
  void pollNativeState();
  metricsTimer = window.setInterval(() => void pollNativeState(), METRICS_POLL_MS);
};

// ─── 临时诊断（定位完即删，合并前必须移除） ──────────────────────
//
// 「收回后只有左上一角」「展开后一大片透明区」这类问题靠推理定不下来：
// 必须知道窗口、画布、各条带各自的**实际矩形**。这里把关键数字和
// 描边直接画到屏幕上，用户截一张图就能定位。
//
// 挂在 document.body 而不是组件里，这样路由切到 /chat 之后它还在
// ——出问题的正是切换之后那一刻。
//
// ⚠️ 合并前必须整段删除（含 DEBUG_FLOATING_OVERLAY 常量与
// showViewportDiagnostic 的全部调用点）。

/** 诊断开关：置 false 即关闭（保留代码便于下次排查）。 */
const DEBUG_FLOATING_OVERLAY = true;

/** 原生报告的窗口宽度（dp），用于和 window.innerWidth 对照。 */
const nativeWindowWidth = ref(0);

const DIAG_VISIBLE_MS = 30000;
let diagTimer: number | undefined;

/** 给元素加一圈描边（outline 不参与布局，不会改变被观测的几何）。 */
const outlineOf = (el: HTMLElement | null, color: string) => {
  if (!el) return;
  el.style.outline = `1px solid ${color}`;
  el.style.outlineOffset = "-1px";
};

/** 撤掉所有诊断描边。 */
const clearOutlines = () => {
  for (const el of [
    document.getElementById("pet-app"),
    avatarContainer.value,
    decorBand.value,
    chatContainer.value,
  ]) {
    if (el) (el as HTMLElement).style.outline = "";
  }
};

/** `w×h @ x,y` 形式的矩形摘要。 */
const rectOf = (el: HTMLElement | null): string => {
  if (!el) return "null";
  const r = el.getBoundingClientRect();
  return `${Math.round(r.width)}x${Math.round(r.height)}@${Math.round(r.left)},${Math.round(r.top)}`;
};

const showViewportDiagnostic = (label: string) => {
  if (!DEBUG_FLOATING_OVERLAY) return;
  // 悬浮窗内**一直**显示：本轮要拿到 innerW 与 nativeW 的对照，判断「展开后
  // 四周空白」到底是视口滞后（innerW ≠ nativeW）还是窗口真的大了。
  // 回到 App 后只在视口明显不对时才显示，修好就自然消失。
  const screenW = window.screen?.width ?? 0;
  const suspicious = floatingWindowMode.value || (screenW > 0 && window.innerWidth < screenW * 0.9);
  if (!suspicious) {
    document.getElementById("__lc_pet_diag")?.remove();
    clearOutlines();
    return;
  }

  // 描边：一眼看出「透明区」到底属于哪条带
  //   品红 = #pet-app 画布，青 = 头像带，黄 = 气泡带，绿 = 输入带
  outlineOf(document.getElementById("pet-app"), "#ff00ff");
  outlineOf(avatarContainer.value, "#22d3ee");
  outlineOf(decorBand.value, "#facc15");
  outlineOf(chatContainer.value, "#4ade80");

  let el = document.getElementById("__lc_pet_diag") as HTMLDivElement | null;
  if (!el) {
    el = document.createElement("div");
    el.id = "__lc_pet_diag";
    // 贴左下角：别盖住宠物本体，截图时才看得见宠物到底多大
    el.style.cssText =
      "position:fixed;left:0;bottom:0;z-index:2147483647;pointer-events:none;" +
      "background:rgba(0,0,0,.8);color:#4ade80;font:11px/1.4 monospace;" +
      "padding:2px 5px;white-space:pre;border-top-right-radius:6px";
    document.body.appendChild(el);
  }
  const expected = Math.round(FLOATING_LOGICAL_WIDTH * floatingFit.value);
  // 角色侧的「桌宠缩放 / 偏移」：这是**桌面端**的调参项，桌面上透明区靠
  // 点击穿透忽略掉，但 Android 悬浮窗没有逐像素穿透——若 scaleP < 1，
  // 宠物就只占头像框的一部分，四周全是吃触摸的透明区。见 GameRoleAvatar。
  const r = gameStore.presentRolesList[0];
  const roleInfo = r
    ? `role scaleP=${r.scaleP} offX=${r.offsetXP} offY=${r.offsetYP} frameless=${r.petFrameless}`
    : "role=none";
  el.textContent =
    `[${label}] inner=${window.innerWidth}x${window.innerHeight} dpr=${window.devicePixelRatio}\n` +
    `fit=${floatingFit.value.toFixed(3)} expectW=${expected} nativeW=${nativeWindowWidth.value}\n` +
    `canvas=${rectOf(document.getElementById("pet-app"))} floating=${floatingWindowMode.value}\n` +
    `avatar=${rectOf(avatarContainer.value)} band=${decorBand.value?.offsetHeight ?? -1}\n` +
    `chat=${rectOf(chatContainer.value)} exp=${petExpanded.value}\n` +
    roleInfo;
  if (diagTimer !== undefined) window.clearTimeout(diagTimer);
  diagTimer = window.setTimeout(() => {
    diagTimer = undefined;
    el?.remove();
  }, DIAG_VISIBLE_MS);
};

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
  // 悬浮窗里装饰带一长高，窗口必须跟着长：气泡被裁掉时用户会以为
  // 「消息发不出去」（实际发出去了，只是回复看不见）。
  // 放在 dy 判定之前——首次观测 dy 为 0，但高度可能已经变了。
  reportFloatingHeight();
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

  // ─── 悬浮窗：固定逻辑画布 + 整体等比缩放 ──────────────────────
  // 窗口尺寸由原生按屏幕比例给，页面则始终按桌面端那套 240dp 宽的布局
  // 排版，再由 `--pet-fit` 整体缩放铺满窗口（见 #pet-app 的 scoped 样式
  // 与 reportFloatingHeight）。
  //
  // 这里刻意**不乘 settingsStore.pet.scale**：手机上的缩放系数由屏幕
  // 比例决定（收起 1/6 屏宽、展开 0.6 屏宽），再乘一次用户缩放会双重缩放。
  // pet.scale 是桌面端「改窗口大小」的概念，悬浮窗里没有对应物。
  if (floatingWindowMode.value) {
    return {
      "--pet-ui-scale": "1",
      "--app-width": `${FLOATING_LOGICAL_WIDTH}px`,
      // 画布高度取「内容需要的高度」，气泡出现时会变高，
      // 否则气泡会把输入框顶出画布、被 overflow-hidden 裁掉。
      "--app-height": `${floatingCanvasHeight.value}px`,
      "--avatar-size": `${AVATAR_BAND_BASE}px`,
      "--chat-h": `${CHAT_BASE_H}px`,
      "--dialog-h": `${DIALOG_MAX_BASE}px`,
      "--pet-fit": floatingFit.value.toString(),
    };
  }

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
let floatingModeUnlisten: (() => void) | null = null;
let expandedUnlisten: (() => void) | null = null;
let metricsUnlisten: (() => void) | null = null;

/**
 * 切到「悬浮窗形态」：页面布局、缩放、轮询三件事一起就位。
 *
 * ## 为什么不能只靠 `isInFloatingWindow()`
 *
 * `MainChat.goToPetMode` 的顺序是「① `router.push('/pet')` → ② `showFloatingPet()`」，
 * 所以本页 `onMounted` 跑的时候第②步还没执行，`isInFloatingWindow()` **必然是 false**。
 * 于是页面掉进**桌面端分支**，真机上表现就是：
 *
 * - `applyWindowLayout()` → `set_pet_mode`（手机上是空操作，但语义已经错了）
 * - 布局用桌面端那套：画布 `PET_WIDTH_BASE × pet.scale`，而且**没有 `--pet-fit`**
 *   整体缩放 → 画布与悬浮窗尺寸对不上，四周空出一大片**吃触摸**的透明区
 * - `GameRolesStage` 的 `frameSize` 乘上 `pet.scale` → 宠物大小由桌面端缩放决定
 * - 渲染出「悬停才浮现」的桌面端按钮
 * - **不启动几何轮询** → 页面永远等不到 `pet-detached` 的自愈
 *
 * 真机反馈「是不是桌面端行为影响了透明区域大小」正是这一条；诊断条不显示
 * 也是因为它挂在轮询里，而轮询压根没起来。
 *
 * 手机端 `/pet` 只可能来自悬浮窗流程（不支持/未授权时 `goToPetMode` 会提前
 * return），所以这里**直接按悬浮窗渲染**，不赌那条不可靠的事件。
 */
const enterFloatingLayout = () => {
  metricsReceived = false;
  lastReportedHeight = -1;
  floatingWindowMode.value = true;
  // 让 api 层的 isInFloatingWindow() 与本页保持一致（进/出都靠它）
  markFloatingWindowMode(true);
  document.body.style.backgroundColor = "transparent";
  document.documentElement.style.backgroundColor = "transparent";
  document.body.style.overflow = "hidden";
  startMetricsPolling();
  // 立刻画一次诊断，不等轮询的第一拍（用户截屏时它必须已经在屏幕上）
  showViewportDiagnostic("enter");
  void nextTick().then(() => reportFloatingHeight());
};

onMounted(async () => {
  floatingWindowMode.value = isInFloatingWindow();

  // 原生搬移/移出悬浮窗时同步本页形态
  floatingModeUnlisten = onFloatingWindowModeChange((active) => {
    // 每次进出都重新等原生的权威几何：搬运瞬间视口宽度是整屏，
    // 自算出来的系数一定是错的。
    metricsReceived = false;
    lastReportedHeight = -1;
    if (active) {
      enterFloatingLayout();
      return;
    }

    // 回到 Activity。原生把 WebView 装回 Activity 时只改了视图父子关系，
    // **路由仍停在 /pet**，必须主动跳回聊天页，否则用户看到的是「桌宠页
    // 铺满整屏、又只渲染出一小块」——既不是聊天界面、也不再是桌宠。
    //
    // 这个事件只是**快路径**：它在搬运/收回前后并不可靠（实测即使在前台
    // 收回也可能收不到），真正兜底的是 pollNativeState 的轮询。
    handleReturnedToApp();
  });

  // 原生改完窗口尺寸后同步展开态
  expandedUnlisten = onPetExpandedChange((expanded) => {
    petExpanded.value = expanded;
  });

  // 原生推来的权威窗口几何。缩放系数**只能**信这个：从 window.innerWidth
  // 自算会踩到「原生刚改完尺寸、WebView 视口还没跟上」的滞后窗口，
  // 算出的系数偏小 → 内容只占窗口一角、展开后一大片空白。
  metricsUnlisten = onPetMetrics(({ scale }) => {
    if (scale > 0) {
      metricsReceived = true;
      floatingFit.value = scale;
    }
    // 系数变了，内容高度的换算结果也变了，立刻按新系数重报一次
    reportFloatingHeight();
  });

  // 手机端：/pet 只可能来自悬浮窗流程，先按悬浮窗形态就位。
  // 必须在下面那条 if 之前——否则会掉进桌面端分支（见 enterFloatingLayout）。
  if (isAndroid() && !floatingWindowMode.value) {
    enterFloatingLayout();
  }

  if (floatingWindowMode.value) {
    // ─── 悬浮窗模式 ────────────────────────────────────────────
    // 与早期「独立 WebView」版本的关键区别：这里**就是主 WebView**，
    // Tauri IPC 完全可用，因此不需要跳过后端调用、也不需要数据镜像。
    // 只做两件悬浮窗专属的事：透明背景，以及跳过桌面端的窗口操作。
    document.body.style.backgroundColor = "transparent";
    document.documentElement.style.backgroundColor = "transparent";
    document.body.style.overflow = "hidden";

    // 逻辑画布 → 窗口的缩放系数，窗口尺寸变化（展开/收起、气泡撑高）时重算
    syncFloatingFit();
    window.addEventListener("resize", onFloatingResize);
    // 开始轮询原生几何：这是缩放系数与「是否已收回」的可靠来源
    startMetricsPolling();
    // 首帧就要把真实内容高度报给原生：原生只知道宽度，收起态/展开态的
    // 初始高度是按同一套常量估的，气泡在挂载时可能已经有内容。
    await nextTick();
    reportFloatingHeight();
    // 注意：这里**不能 return**。IPC 可用意味着角色数据、语音、
    // 对话推进等全部逻辑都能正常工作——这正是搬运方案的价值。
  } else {
    // 桌面端：调整原生窗口为桌宠尺寸
    await applyWindowLayout().catch(() => {});
  }

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

  // ─── 悬浮窗模式到此为止 ─────────────────────────────────────
  // 上面的监听器都要保留（它们只更新 store，与窗口无关），
  // 但下面两步是**桌面端窗口专属**的，在悬浮窗里必须跳过：
  //
  // - applyWindowLayout → set_pet_mode 会把**整个 Activity 窗口**缩成桌宠尺寸，
  //   而悬浮窗尺寸已由原生按屏幕比例定好，再调会互相打架
  // - hitTestInterval → 桌面端靠它做逐像素点击穿透；Android 的穿透是窗口级开关，
  //   这套 solid region 上报在手机上没有任何作用，只会 10Hz 空转唤醒后端
  if (floatingWindowMode.value) {
    // 悬浮窗里单击头像即展开，不依赖光标位置（手机没有 hover）
    showChatInput.value = false;
    return;
  }

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
  // 悬浮窗模式下由本组件设置的滚动锁（见 onMounted 的早期返回分支）
  document.body.style.overflow = "";

  if (scaleUnlisten) scaleUnlisten();
  if (effectUnlisten) effectUnlisten();
  if (volumeUnlisten) volumeUnlisten();
  if (live2dFpsUnlisten) live2dFpsUnlisten();
  if (dialogHistoryUnlisten) dialogHistoryUnlisten();
  if (cursorUnlisten) cursorUnlisten();
  if (bubbleSideUnlisten) bubbleSideUnlisten();
  if (movedUnlisten) movedUnlisten();
  if (floatingModeUnlisten) floatingModeUnlisten();
  if (expandedUnlisten) expandedUnlisten();
  if (metricsUnlisten) metricsUnlisten();
  if (autoSideTimer !== undefined) window.clearTimeout(autoSideTimer);
  if (heightReportTimer !== undefined) window.clearTimeout(heightReportTimer);
  if (returningTimer !== undefined) window.clearTimeout(returningTimer);
  stopMetricsPolling();
  window.removeEventListener("resize", onFloatingResize);
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

/**
 * 点击头像。
 *
 * - **悬浮窗**：展开 / 收起**来回切换**
 * - **桌面端**：推进对话（原有行为）
 *
 * 手机上不用「展开后另给一个收起按钮」那套：悬浮窗里页面是整体缩放的，
 * 挂在头像角上的小圆按钮会被 `overflow-hidden` 裁掉一半，实测点不到。
 * 直接把同一个手势做成开关，既省掉一个可能落在窗口外的热区，
 * 也省掉一次「按钮在不在窗口内」的布局风险。
 *
 * 代价是悬浮窗展开态下不能再点头像推进对话——但那时用户有输入框，
 * 推进对话由发送消息 / 自动模式承担，比在手机上误触收起要好。
 */
const handleAvatarClick = () => {
  if (floatingWindowMode.value) {
    void (petExpanded.value ? collapsePet() : expandPet());
    return;
  }

  // 走 continueDialog 而非直接 eventQueue.continue()：后者绕过了「打字中先补全文本
  // 再推进」的守卫，也跳过 player-continued/dialog-proceed 派发——打字中点头像会
  // 把正在打的字丢掉。continueDialog 在真的推进时会派发 player-continued，
  // 由 @player-continued 绑定的 manualTriggerContinue 取消待触发的自动推进定时器。
  gameDialogRef.value?.continueDialog(true);
};

/** 展开悬浮窗：显示输入框，窗口同步变大（原生按屏幕比例算，约 0.6 屏宽）。 */
const expandPet = async () => {
  // 先乐观改本地状态：原生的 pet-metrics 会在 invoke 返回前后到达，
  // 那时 DOM 必须已经按展开态排好版，reportFloatingHeight 才量得到正确高度。
  petExpanded.value = true;
  showChatInput.value = true;
  try {
    await setFloatingPetExpanded(true);
  } catch (e) {
    petExpanded.value = false;
    showChatInput.value = false;
    console.error("[PetMode] 展开悬浮窗失败:", e);
    return;
  }
  // 兜底：万一本轮 pet-metrics 丢了，也要把新高度报上去
  scheduleHeightReport();
};

/** 收起悬浮窗：隐藏输入框，回到仅头像形态。 */
const collapsePet = async () => {
  petExpanded.value = false;
  showChatInput.value = false;
  try {
    await setFloatingPetExpanded(false);
  } catch (e) {
    petExpanded.value = true;
    showChatInput.value = true;
    console.error("[PetMode] 收起悬浮窗失败:", e);
    return;
  }
  scheduleHeightReport();
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
  // 悬浮窗模式：**先**把页面本地切回正常布局，再去调原生命令搬回去。
  //
  // 顺序很重要。若反过来「先 await 原生、再靠事件通知页面归位」，一旦那条
  // 事件丢了（实测在搬运/收回前后会丢），页面就永远停在悬浮窗分支——用户
  // 看到的就是「收回了，但只有左上一角」。现在页面自己立即归位，原生那边
  // 成不成功都不影响界面正确性。
  //
  // 手机端额外兜一层：万一形态判断出错（页面以为自己在桌面端），也必须让
  // 原生把悬浮窗摘掉，否则窗口会一直留在屏幕上、且再也关不掉。
  if (floatingWindowMode.value || isAndroid()) {
    if (!floatingWindowMode.value) {
      floatingWindowMode.value = true;
      markFloatingWindowMode(true);
    }
    handleReturnedToApp();
    try {
      await hideFloatingPet();
    } catch (e) {
      console.error("[PetMode] 退出悬浮窗失败:", e);
    }
    return;
  }

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
/*
 * 尺寸必须走 var(--app-width/height)，不能写 100vw/100dvh：
 * ID 选择器的优先级高于 Tailwind 工具类，写成 100vw/100dvh 会把模板上的
 * `w-(--app-width) h-(--app-height)` 全部压掉，悬浮窗里页面就永远是
 * 「满视口」而不是「逻辑画布」，整体缩放随之失效。
 *
 * --pet-fit 只在悬浮窗模式下有值（= window.innerWidth / 240），
 * 桌面端缺省 1，缩放是恒等变换。
 */
#pet-app {
  position: relative;
  width: var(--app-width, 100vw);
  height: var(--app-height, 100dvh);
  overflow: hidden;
  transform: scale(var(--pet-fit, 1));
  transform-origin: top left;
}
</style>
