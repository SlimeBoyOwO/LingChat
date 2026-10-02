<template>
  <!--
    外壳层：**恰好等于 WebView 视口**。

    `#app` 是 `position: fixed; width: 100dvw; height: 100dvh` —— 它读的就是
    WebView 的实际视口尺寸。本层 `inset: 0` 套在它里面，因此本层的尺寸
    **就是** WebView 的尺寸，不经过任何计算、不依赖任何原生上报。

    这一层存在的意义就是把「外面」这件事钉死：无论里层的逻辑画布算成什么，
    外面这一层永远是窗口本身，不可能比窗口小、不可能在外面露出一圈透明；
    里层万一算大了也只是被这里 `overflow: hidden` 裁掉。

    里层 `#pet-app` 仍是「固定逻辑画布 + 整体等比缩放」，但它现在被钉在一个
    尺寸恒等于窗口的盒子里 —— 它的渲染宽 = `--app-width × --pet-fit`
    = `210 × (视口宽 / 210)` ≡ 视口宽，是**恒等式**，不是估算。
  -->
  <div id="pet-shell">
    <div
      id="pet-app"
      v-show="!returningToApp"
      :style="appStyleVars"
      @mouseenter="handleMouseEnter"
      @mouseleave="handleMouseLeave"
      class="relative flex h-(--app-height) w-(--app-width) flex-col items-center justify-start overflow-hidden bg-transparent transition-none select-none"
    >
      <!-- 这里**不再**单放一个「返回」按钮。
           桌面端本来就有一排左侧圆形按钮（设置 / 自动 / 返回主页 / 截图 /
           麦克风，见 GameRolesStage.vue），「返回主页」就在里面。悬浮窗里需要
           做的只是让那一排别被 `v-if="!floatingMode"` 隐藏、别被
           `#pet-app` 的 overflow-hidden 裁掉（见 GameRolesStage 的
           `sideButtonClass`），而不是另造一个图标和位置都不同的按钮 ——
           同一件事有两套入口，迟早对不上。 -->

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
            :floating-window="floatingWindowMode"
            :expanded="petExpanded"
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
      <div
        class="w-full flex-1"
        :style="{ order: floatingWindowMode || bubbleBelow ? 3 : 0 }"
      ></div>
    </div>
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
import { isAndroid } from "@/utils/platform";
import {
  AVATAR_BAND_BASE,
  CHAT_BASE_H,
  DIALOG_MAX_BASE,
  FLOATING_LOGICAL_WIDTH,
  PET_WIDTH_BASE,
} from "../pet/constants";

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
 * 看起来和「没收回去」一模一样。`router.replace("/chat")` 落地通常只要几十
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
// 悬浮窗里的页面**不做响应式布局**：始终按同一套逻辑画布（宽度见
// `FLOATING_LOGICAL_WIDTH`，= 头像带宽 210dp）排版，再整体
// `transform: scale(窗口宽度 / 210)` 缩放到窗口。
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
// ⚠️ 逻辑宽度**不能**用桌面端的 `PET_WIDTH_BASE`(240)：那 240 里有
// 两侧各 15px 是给桌面端悬停按钮与光晕留的「呼吸边」，悬浮窗里按钮
// 全被隐藏 → 那 15px 就是纯透明空白，头像被居中后左右各露出一圈
// （展开态单侧 13.5px ≈ 2.9mm，肉眼一眼可见）。详见 constants.ts
// 里 `FLOATING_LOGICAL_WIDTH` 的实测数字。
//
// 代价：文字绝对大小与窗口宽度成正比，所以展开态不能太窄——见
// FloatingPetPlugin.kt 的 EXPANDED_WIDTH_RATIO（取 0.6 屏宽，缩放系数约 1.03）。

/** 逻辑画布 → 实际窗口的缩放系数。仅悬浮窗模式有意义。 */
const floatingFit = ref(1);

/**
 * 逻辑画布的内容高度（未缩放）。
 *
 * 由 {@link reportFloatingHeight} 从实际 DOM 量出来回填——气泡是流式
 * 输出的，高度随时在变，写死常量必然算错。初值取收起态的头像带高度。
 */
const floatingContentHeight = ref(AVATAR_BAND_BASE);

/**
 * 视口尺寸（CSS px）。
 *
 * `window.innerWidth/innerHeight` 是**非响应式**的，直接写进 `computed`
 * 里不会触发重算。这里把它们同步进一个 ref，让画布高度能跟着视口变。
 *
 * 同步点：`onMounted`、`resize` 事件、进悬浮窗、自愈心跳（见
 * {@link startSelfHeal}）。心跳那条最关键——它是唯一不依赖任何原生通道的
 * 兜底，视口没变过时 `resize` 一次都不会来。
 */
const viewportSize = ref({ w: 0, h: 0 });
const syncViewportSize = () => {
  const w = window.innerWidth;
  const h = window.innerHeight;
  if (w === viewportSize.value.w && h === viewportSize.value.h) return;
  viewportSize.value = { w, h };
};

/**
 * **当前**缩放系数 —— 直接从视口尺寸现算，不经过任何缓存。
 *
 * ## 这是「外面那一圈」的结构性根治
 *
 * `--app-width` 是常量 210，`--pet-fit` 由本函数从**视口宽**现算，
 * 两者在**同一次渲染**里取值，于是
 *
 * ```
 * 画布渲染宽 = --app-width × --pet-fit = 210 × (视口宽 / 210) ≡ 视口宽
 * ```
 *
 * 恒成立 —— 画布**不可能**比 WebView 窄，也就**不可能**在外面露出一圈。
 *
 * ## 早先错在哪
 *
 * 早先这里读的是 `floatingFit` 那个 ref，而它由三条**异步**通道更新
 * （原生推 `pet-metrics` / 500ms IPC 轮询 / `resize` 事件），最多滞后
 * 500ms。在滞后的那段时间里画布是按**旧**系数渲染的：视口已经变成
 * 216 宽，画布还按收起态的 0.25 渲染成 52 宽 —— 右边 164px 全是透明。
 * 这不是「算错了」，而是「用了过期的值」。
 *
 * 现在 `floatingFit` 只作为「视口尺寸还没同步进来」（`viewportSize.w === 0`，
 * 即 `syncViewportSize` 一次都没跑过）时的兜底。
 */
const liveFit = computed(() => {
  const w = viewportSize.value.w;
  const fromViewport = w > 0 ? w / FLOATING_LOGICAL_WIDTH : 0;

  // ── 视口还没跟上窗口变化时，改用原生**即时**推来的权威系数 ──────
  //
  // 展开 / 折叠会改窗口尺寸，而 WebView 的**视口**要过一会儿才跟着变。
  // 这段窗口里 `fromViewport` 还是旧值：折叠时窗口已经缩到约 1/6 屏宽，
  // 画布却仍按展开态的大系数渲染 → 比窗口大一大截 → 内容被裁掉一块，
  // 几帧后才缩回来。用户看到的就是「折叠时闪一下」。
  //
  // 原生的 `pet-metrics` 是**改完窗口立刻**推的（与窗口尺寸同源），所以这段
  // 时间用它。视口一跟上两者就相等，自动切回现算 —— 而现算正是「画布宽 ≡
  // 视口宽」那条恒等关系的来源（见本函数的说明），不能丢。
  //
  // 两个条件都要满足才切：`authoritativeAt` 限定「刚刚推过」（避免用 500ms
  // 轮询的滞后值），相对差 2% 限定「确实对不上」（避免稳定态下的浮点抖动）。
  const auth = floatingFit.value;
  if (
    auth > 0 &&
    fromViewport > 0 &&
    Date.now() - authoritativeAt < AUTHORITATIVE_TTL_MS &&
    Math.abs(fromViewport - auth) / auth > 0.02
  ) {
    return auth;
  }

  if (fromViewport > 0) return fromViewport;
  return auth > 0 ? auth : 1;
});

/**
 * 最近一次收到原生**即时**推来的权威几何（`pet-metrics`）的时间戳。
 *
 * 见 [liveFit]：窗口尺寸变化后的几百毫秒内，WebView 视口还没跟上，
 * 现算会拿到旧系数，必须先用原生推来的那个。
 */
let authoritativeAt = 0;

/** [authoritativeAt] 的有效期（毫秒）。超过就无条件切回「从视口现算」。 */
const AUTHORITATIVE_TTL_MS = 400;

/**
 * 画布逻辑高度。
 *
 * = `max(内容需要的高度, 视口高度 ÷ fit)`
 *
 * 后一项让画布**在任何方向都不小于视口** —— 窗口是矩形、画布是矩形，
 * 只要画布比窗口小，就会露出一圈透明（用户报的「外面那一圈」）。
 * 画布比视口大是**无害**的：多出来的部分由末尾那条 `flex-1` 余量带
 * 吸收，内容仍然顶部对齐。
 *
 * ## 为什么宽度不用同一套「不小于视口」的约束
 *
 * 宽度是**构造出来**的：`fit = 视口宽 / FLOATING_LOGICAL_WIDTH`，
 * 所以 `逻辑宽 × fit ≡ 视口宽`，恒等成立、不需要再兜。
 * 高度做不到这一点——它由内容决定（气泡是流式输出的），
 * 所以必须显式取 max 兜住「窗口比内容高」那一半。
 *
 * 这里用的 `fit` 必须是 {@link liveFit}（现算），不能是 `floatingFit`
 * （缓存）——否则画布高度会按过期系数算，和宽度对不上。
 *
 * 注意上报给原生的仍然是**内容高度**（见 reportFloatingHeight），
 * 所以窗口高度由内容决定，不会和视口形成
 * 「改高度 → 视口变 → 再改高度」的来回震荡。
 */
const floatingCanvasHeight = computed(() => {
  const base = petExpanded.value ? AVATAR_BAND_BASE + CHAT_BASE_H : AVATAR_BAND_BASE;
  const content = Math.max(base, floatingContentHeight.value);
  if (!floatingWindowMode.value) return content;
  const fit = liveFit.value;
  const vh = viewportSize.value.h;
  if (!(fit > 0) || !(vh > 0)) return content;
  return Math.max(content, vh / fit);
});

// 形态一变就把量到的内容高度作废。
//
// `floatingContentHeight` 是「上一次量到的内容高度」，它只在
// reportFloatingHeight 里被写。如果形态切换后那一次测量没跑成
// （原生通道失效、DOM 还没排好版），画布高度会**留在上一个形态的值**上：
// 展开态量到 280（或气泡撑到 480）之后收起，画布仍按 480 撑着 ——
// 窗口只有 52dp 高，画布却是 480×0.25=120dp，下方多出来的一条
// 在窗口里就是透明带（窗口是矩形，画布不是）。
//
// 作废后 `floatingCanvasHeight` 立刻回落到新形态的基准值，等真正的
// 测量结果到达再修正。
watch(petExpanded, () => {
  floatingContentHeight.value = AVATAR_BAND_BASE;
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
  // Android 上这条分支是**故意关掉**的：它只在「改完布局立刻读」的场合
  // 被调用（onMounted），读到的是滞后值。悬浮窗里系数一律由
  // onFloatingResize / startSelfHeal 在视口落定之后算（见 fitForViewport）。
  if (isAndroid()) return;
  const k = window.innerWidth / FLOATING_LOGICAL_WIDTH;
  if (k > 0) floatingFit.value = k;
};

/**
 * 按**视口宽度**算缩放系数。`fit = window.innerWidth / FLOATING_LOGICAL_WIDTH`。
 *
 * 与 {@link syncFloatingFit} 的区别是**调用时机**，不是算法：
 * 那个函数是在「改完布局立刻读」的场合被调用的，读到的是滞后值；
 * 本函数只在视口尺寸**刚刚确定**之后调用。
 *
 * ## 为什么只取宽度、**不**再对高度取 min
 *
 * 画布的视觉高度是 `逻辑高 × fit`；而窗口高度由原生按**同一个**
 * `逻辑高 × fit` 反算出来（`set_size` 的 `logicalHeight` 口径，见
 * FloatingPetPlugin.setSize）。也就是说
 * 「画布视觉高度 == 窗口高度」是**构造出来**的，页面不需要、也不应该
 * 再自己约束一次高度。
 *
 * 早先这里写的是 `min(byWidth, byHeight)`，本意是防「系统把视口压矮、
 * 内容溢出被顶到屏幕上方」。但它把系数压小的同时，**画布宽度也跟着
 * 小于窗口宽度** —— 于是窗口右侧、下侧各留一条透明带，用户看到的就是
 * 「展开后一大片空白」。
 *
 * 真机诊断实测（诊断浮层的 `gap` 一行）：
 *
 * ```
 * inner=360x802  canvas=240x480@0,0  fit=1.000
 * gap L0 T0 R120 B322 <== 空白!      // 360-240=120，803-480=323
 * ```
 *
 * 而且那条透明带**照样吃触摸**：它落在 `#pet-app` 之外、窗口之内，
 * 手指点上去会触发 `mouseleave`，把输入框收起来 —— 用户报的
 * 「展开后按空白区域会触发输入框折叠」就是这么来的。
 *
 * 现在只认宽度：**画布永远铺满窗口宽度**，无论窗口尺寸是原生给的、
 * 系统改的，还是页面自己读到的。
 */
const fitForViewport = (): number => {
  const w = window.innerWidth;
  if (!(w > 0)) return 0;
  return w / FLOATING_LOGICAL_WIDTH;
};

/**
 * 把可能被系统顶上去的内容归位。
 *
 * 输入法弹出时浏览器会平移视口（visual viewport）去露出被聚焦的元素，
 * 而 `#app` 是 `position: fixed` —— 固定定位元素会跟着视口一起平移，
 * 于是整个画布被顶到屏幕上方。这里在每次 resize 后把它拉回来。
 */
const resetViewportPan = () => {
  try {
    if (window.scrollX !== 0 || window.scrollY !== 0) window.scrollTo(0, 0);
  } catch {
    // 忽略：某些 WebView 在文档不可滚动时会抛
  }
};

/**
 * 悬浮窗尺寸变化（展开/收起、气泡撑高、系统改窗口、旋转）后重算并回报。
 *
 * 这里是**唯一**不依赖原生 `evaluateJavascript` 的通道：原生推事件那条路
 * 在搬运/回收前后并不可靠，而 `resize` 是浏览器自己派发的。
 *
 * ⚠️ 调用方一律走 {@link onViewportResize}（它先同步视口再进这里），
 * 因为 `floatingCanvasHeight` 依赖 `viewportSize`。
 */
let fitRafId: number | undefined;
const onFloatingResize = () => {
  if (!floatingWindowMode.value) return;
  // 先同步视口：`floatingCanvasHeight` 依赖它（画布必须不小于视口），
  // 顺序反了会先用上一帧的视口算一次高度。
  syncViewportSize();
  resetViewportPan();
  const k = fitForViewport();
  if (k > 0) {
    floatingFit.value = k;
    // 视口实测值比原生推来的值更贴近「用户真正看得见的区域」，采信它。
    // 同时把 metricsReceived 置真，避免 syncFloatingFit 再用更差的来源覆盖。
    metricsReceived = true;
  }
  reportFloatingHeight();

  // ── 再等一帧复核一次 ────────────────────────────────────────
  // 原生 `updateViewLayout` 之后 WebView 的视口要下一帧才更新；连续两次
  // 尺寸变化（收起→展开、气泡撑高→回落）时，第一帧读到的可能还是中间值。
  // 一帧后复核，成本可忽略，但能把「差一点点」的系数纠回来。
  if (fitRafId !== undefined) return;
  fitRafId = window.requestAnimationFrame(() => {
    fitRafId = undefined;
    if (!floatingWindowMode.value) return;
    const k2 = fitForViewport();
    if (k2 > 0 && Math.abs(k2 - floatingFit.value) > 1e-4) {
      floatingFit.value = k2;
      reportFloatingHeight();
    }
  });
};

/**
 * `resize` 事件的统一入口：**先同步视口，再走悬浮窗重算**。
 *
 * `window.innerWidth/innerHeight` 不是响应式的，`floatingCanvasHeight`
 * 只认 `viewportSize`。两件事拆成两个监听器就会漏掉其中一个
 * （早先 `resize` 只挂了 `onFloatingResize`，视口 ref 永远是初值 0）。
 */
const onViewportResize = () => {
  syncViewportSize();
  onFloatingResize();
};

/**
 * 本地自愈心跳：**不依赖任何原生通道**地把系数拉回正确值。
 *
 * ## 为什么必须有
 *
 * `fit` 只有三个来源，且都可能失效：
 *
 * | 来源 | 失效场景 |
 * |---|---|
 * | 原生推 `pet-metrics`（`evaluateJavascript`） | WebView 刚重挂 / 宿主在后台时整体丢失 |
 * | 原生轮询 `status`（500ms IPC） | 任一环节失败就永远是初值 |
 * | `resize` 事件 | 只在视口尺寸**变化**时才有；视口没变过就一次都不来 |
 *
 * 三者同时失效是**真实发生过**的。真机诊断浮层抓到过这样一帧：
 *
 * ```
 * [enter] inner=360x802  win=0x0dp  fit=1.000 applied=1.00
 * canvas=240x480@0,0     gap L0 T0 R120 B322 <== 空白!
 * ```
 *
 * `win=0x0dp` 说明 IPC 那条路一次都没成功（`fit` 停在初值 1.000），
 * 而视口从未变过 → `resize` 一次都没派发 → 页面**永远**停在
 * 「240×480 的画布铺在 360×802 的视口里」，右边和下边全是空白。
 *
 * 这个心跳只读 `window.innerWidth`、不碰 IPC，因此**只要页面还在跑，
 * 系数就一定会收敛到「画布铺满视口宽度」**，不需要任何原生配合。
 * 收敛之后每次心跳只是一次 `Math.abs` 比较，成本可忽略。
 */
const SELF_HEAL_INTERVAL_MS = 500;
let selfHealTimer: number | undefined;
const startSelfHeal = () => {
  if (selfHealTimer !== undefined) return;
  selfHealTimer = window.setInterval(() => {
    if (!floatingWindowMode.value) return;
    // 视口可能变过而 resize 没派发（原生改窗口、转屏、输入法收起）——
    // 心跳顺手把它同步进 ref，`floatingCanvasHeight` 才会跟着重算。
    syncViewportSize();
    const k = fitForViewport();
    if (k > 0 && Math.abs(k - floatingFit.value) > 1e-3) {
      // 系数不对：走完整路径（重算 + 回报高度）
      onFloatingResize();
      return;
    }
    // 系数对了，但内容高度可能变了（气泡流式输出），补一次上报。
    // reportFloatingHeight 自带去重，没变化时不会产生 IPC。
    reportFloatingHeight();
  }, SELF_HEAL_INTERVAL_MS);
};
const stopSelfHeal = () => {
  if (selfHealTimer === undefined) return;
  window.clearInterval(selfHealTimer);
  selfHealTimer = undefined;
};

/**
 * 把内容高度上报给原生，让窗口恰好裹住内容。
 *
 * 原生只知道宽度（按屏幕比例算），高度得由页面说了算——气泡出现时
 * 内容会变高，窗口必须跟着长，否则气泡被裁掉、用户以为「消息发不出去」。
 *
 * ## 上报的是「逻辑高度」，不是「实际 dp」
 *
 * 传 `logicalHeight` 让**原生**按它手里的权威窗口宽度换算实际高度。
 * 早先这里传的是 `Math.round(logical * k)`（实际 dp），而 `k` 是页面
 * 手里的缩放系数——它可能过期（原生刚 `updateViewLayout` 完、`pet-metrics`
 * 还没送达）。那一刻页面会拿「收起态的 k」乘「展开态的逻辑高度」，
 * 把一个 70dp 的高度写进一个已经展开到 216dp 宽的窗口：宽度对了、高度
 * 塌了，剩下的区域就是用户看到的「大片空白」。
 *
 * 改报逻辑高度后，窗口高度与宽度**在构造上**由原生保证一致，
 * 这条竞态从根上消失。
 *
 * 高度只依赖内容（头像带 + 输入带 + 气泡带），**不依赖窗口高度**，
 * 因此不会和原生形成「改高度 → 重排 → 再改高度」的来回震荡。
 * `lastReportedHeight` 再去掉重复上报。
 */
let lastReportedHeight = -1;
const reportFloatingHeight = () => {
  if (!floatingWindowMode.value) return;
  const base = petExpanded.value ? AVATAR_BAND_BASE + CHAT_BASE_H : AVATAR_BAND_BASE;
  // offsetHeight 是布局尺寸（未乘 transform），正是逻辑画布里的高度
  const band = decorBand.value?.offsetHeight ?? 0;
  const logical = base + band;
  // 先让画布长高再报尺寸：反过来的话，窗口先变大而画布还是旧的，
  // 中间那一帧气泡会把输入框顶出画布、被 overflow-hidden 裁掉。
  floatingContentHeight.value = logical;
  const logicalRounded = Math.round(logical);
  if (logicalRounded <= 0) return;
  // 容差 1px（逻辑像素）：气泡是流式输出的，每次多一两个字都会让高度抖 1px。
  // 没有容差就会和原生来回改尺寸停不下来。
  if (lastReportedHeight > 0 && Math.abs(logicalRounded - lastReportedHeight) <= 1) return;
  lastReportedHeight = logicalRounded;
  // 宽度传 0 = 「只改高度」：宽度归原生独占。
  void resizeFloatingPet(0, 0, logicalRounded).catch(() => {
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
 * 连续观察到「原生说 WebView 已不在悬浮窗里」的轮数。
 *
 * 收回是**不可逆**的（`handleReturnedToApp` 会 `router.replace("/chat")`
 * 并停掉轮询），所以不能凭单轮读数下结论：搬运 / 重排 / 旋转期间主线程
 * 忙着布局，查询本身可能失败或拿到中间态。连续 [RETURNED_CONFIRM_ROUNDS]
 * 轮（间隔 500ms）都是 false 才认。
 *
 * 正常收回有 `pet-attached` 事件做快路径，轮询只是兜底，多等一轮无感。
 */
let returnedStreak = 0;

/** 收回判定需要连续确认的轮数。见 [returnedStreak]。 */
const RETURNED_CONFIRM_ROUNDS = 2;

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
  stopSelfHeal();
  returnedStreak = 0;
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
  void router.replace("/chat").then(() => {
    // 导航没落地（页面还停在 /pet）→ 形态已经切成「桌面端」，而窗口还是
    // 悬浮窗那个小矩形，用户看到的就是「角色凭空消失、窗口还在」。
    // 这是**唯一**能自愈的地方：MainChat 没挂载，它那条兜底跑不到。
    // 回滚形态并让轮询继续跑，等下一次真收回时再切。
    if (router.currentRoute.value.path === "/pet") {
      console.warn("[PetMode] 切回聊天页未生效，回滚为悬浮窗形态");
      enterFloatingLayout();
    }
  });
};

/**
 * 复核一条「已回到 App」的信号，确认后才真的收回。
 *
 * ## 为什么不能直接相信 `pet-attached`
 *
 * 原生在收回后会**重发 4 次** `pet-attached`（0/300/1000/3000ms，见 Kotlin
 * 侧的 `PET_ATTACHED_RETRY_DELAYS_MS`）——那串重试是为「用户是在别的 App
 * 里收回的、WebView 当时收不到 JS」准备的。但用户**很快又重新进悬浮窗**
 * 时它就成了毒药：
 *
 * ```
 * t=0.0  点 ✕ 收回 → 排队 [0, 300, 1000, 3000]
 * t=1.5  又点「启动桌宠」→ 重新搬进悬浮窗，页面重新挂载、监听器重新绑上
 * t=3.0  最后一条陈旧的 pet-attached 到达 → 命中**新一轮**的监听器
 *        → 页面以为用户收回了桌宠 → push /chat + 停轮询（不可逆）
 *        → 悬浮窗里变成聊天页、角色凭空消失
 * ```
 *
 * 这就是「反复切来切去就卡成聊天页」的直接成因。原生侧已按轮次作废这类
 * 陈旧事件（见 `petModeEpoch`），这里是**第二道防线**：即使事件真的发出来
 * 了（evaluateJavascript 已经执行、无法撤回），也回问原生一次再决定。
 *
 * 查询失败时**什么都不做** —— 宁可晚一轮收回，也不能凭猜测把用户踢走。
 */
const confirmReturnedToApp = async () => {
  const status = await getFloatingPetStatus();
  if (!status.queried) return;
  if (status.detached) {
    // 原生说 WebView 还在悬浮窗里 → 这是上一轮的残响，丢弃。
    // api 层的标记已被事件回调置成 false，这里改回来。
    markFloatingWindowMode(true);
    if (!floatingWindowMode.value) enterFloatingLayout();
    return;
  }
  handleReturnedToApp();
};

const stopMetricsPolling = () => {
  if (metricsTimer === undefined) return;
  window.clearInterval(metricsTimer);
  metricsTimer = undefined;
};

/** 查询一次原生状态；查询失败保持现状，等下一轮。 */
const pollNativeState = async () => {
  // 桌面端不轮询。Android 上**无论当前是什么形态**都要问：形态本身
  // 就是靠这个答案决定的（见 onMounted 里 startMetricsPolling 的说明）。
  if (!isInFloatingWindow() && !isAndroid()) return;
  try {
    const status = await getFloatingPetStatus();
    // ── 查询失败：这一轮不作数 ────────────────────────────────
    //
    // 绝不能把「没查到」当成「已经回到 App」。`handleReturnedToApp` 会
    // push /chat 并停掉轮询，一旦误判就再也回不来（真机表现：悬浮窗
    // 卡成聊天页、角色凭空消失）。搬运 / 旋转重排期间主线程忙着布局，
    // 查询失败恰恰是最常见的时候 —— 也就是「反复切来切去」时。
    if (!status.queried) return;
    if (status.detached) {
      sawDetached = true;
      returnedStreak = 0;
      // 兜底自愈：万一进悬浮窗时的事件丢了、页面还停在桌面端布局
      // （见 enterFloatingLayout 的说明），这里按原生的权威答案切回来。
      if (!floatingWindowMode.value) enterFloatingLayout();
      if (status.scale > 0) {
        metricsReceived = true;
        floatingFit.value = status.scale;
      }
      reportFloatingHeight();
      return;
    }
    // 原生说 WebView 已经不在悬浮窗里了 → 按「已回到 App」处理。
    // 只有**见过** detached 才认，否则会误伤「刚挂载、还没搬进去」。
    if (!sawDetached) return;
    // 连续确认，避免把中间态当成最终态。见 [returnedStreak]。
    returnedStreak += 1;
    if (returnedStreak >= RETURNED_CONFIRM_ROUNDS) handleReturnedToApp();
  } catch {
    // 插件不可用或瞬时失败，下一轮重试
  }
};

const startMetricsPolling = () => {
  if (metricsTimer !== undefined) return;
  void pollNativeState();
  metricsTimer = window.setInterval(() => void pollNativeState(), METRICS_POLL_MS);
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
  // 窗口尺寸由原生按屏幕比例给，页面则始终按那套逻辑画布（210dp 宽）的布局
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
      // 必须用 liveFit（现算），不能用 floatingFit（缓存）：
      // 见 liveFit 的注释 —— 缓存值滞后会让画布比窗口小，露出一圈透明。
      "--pet-fit": liveFit.value.toString(),
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
 * 监听**根元素**尺寸变化，把 WebView 视口尺寸实时同步进 `viewportSize`。
 *
 * 为什么不只靠 `window.resize`：在 Android WebView 里，窗口被原生改尺寸
 * （`updateViewLayout`）、转屏、输入法弹出/收起时，`resize` 事件**并不保证
 * 派发**（实测抓到过「视口已经从 360 变成 216，页面一次 resize 都没收到」）。
 * `ResizeObserver` 观察的是布局结果本身，比窗口事件可靠得多。
 *
 * 这一条是 `liveFit` 的数据源：只要它同步上了，`--pet-fit` 就一定是
 * 「视口宽 / 210」，画布就永远铺满 WebView。
 */
let viewportObserver: ResizeObserver | undefined;

/**
 * 切到「悬浮窗形态」：页面布局、缩放、轮询三件事一起就位。
 *
 * ## 为什么形态不能由事件或平台假设决定
 *
 * `MainChat.goToPetMode` 的顺序是「① `router.replace('/pet')` → ② `showFloatingPet()`」，
 * 所以本页 `onMounted` 跑的时候第②步还没执行，`isInFloatingWindow()` **必然是 false**。
 *
 * - 如果就此按「桌面端」渲染，搬移完成后页面**不会自己切回来**（`pet-detached`
 *   不可靠），真机表现是：布局用 `PET_WIDTH_BASE × pet.scale`、没有 `--pet-fit`
 *   整体缩放、渲染出桌面端悬停按钮、✕ 走桌面端退出路径（**不调 `hideFloatingPet`**，
 *   悬浮窗永远留在屏幕上）。
 * - 反过来，如果因为「手机端 /pet 必然是悬浮窗」就在挂载时**抢跑**成悬浮窗形态，
 *   那搬移完成前页面会以「宠物画布尺寸」渲染在一个**整屏** WebView 里
 *   ——真机诊断实测 `inner=360x802 floating=true canvas=360x315@0,0 nativeW=0`，
 *   也就是屏幕下方空出四百多 dp。
 *
 * 两条路都不对，唯一正确的答案是**问原生**：`status.detached`。轮询在挂载时
 * 就启动，答案一到就切形态；切过来之前老老实实按整屏布局渲染。
 */
const enterFloatingLayout = () => {
  metricsReceived = false;
  lastReportedHeight = -1;
  // 新一轮开始：清掉上一轮残留的收回确认计数（见 returnedStreak）
  returnedStreak = 0;
  floatingWindowMode.value = true;
  // 视口尺寸先落进 ref：`floatingCanvasHeight` 用它兜「画布不小于视口」，
  // 这里不刷的话第一次 computed 会拿 0 当视口高、跳过那层兜底。
  syncViewportSize();
  // 让 api 层的 isInFloatingWindow() 与本页保持一致（进/出都靠它）
  markFloatingWindowMode(true);
  document.body.style.backgroundColor = "transparent";
  document.documentElement.style.backgroundColor = "transparent";
  document.body.style.overflow = "hidden";
  document.documentElement.style.overflow = "hidden";
  // ── resize 通道 ─────────────────────────────────────────────
  //
  // 监听器在 `onMounted` 里**无条件**绑上（见 onViewportResize 的说明），
  // 所以这里不再重复绑。早先只在「挂载时已经是悬浮窗」的分支里绑，
  // 而按本页的启动顺序（`goToPetMode` 是先 `router.replace('/pet')` 再
  // `showFloatingPet()`），那条分支**永远走不到** —— 整条 resize 通道是死的，
  // 窗口变化时页面只能等原生轮询，展开后画布停在收起态大小、挤在左上角。
  startMetricsPolling();
  // 本地自愈心跳：即使原生推事件与 500ms 轮询两条路都失效，也能靠
  // `window.innerWidth` 把系数拉回来。见 startSelfHeal 的说明。
  startSelfHeal();
  void nextTick().then(() => {
    // 第一帧就把系数按当前视口算出来：`pet-detached` 是 addView 之后立刻
    // 派发的，此刻 WebView 视口往往还是搬运前那个（整屏）值。先按它算一次，
    // 免得在「画布只有 210 宽、视口却有 360 宽」那一帧留下大片空白；
    // 真正的值由 resize / 心跳在视口跟上后修正。
    onViewportResize();
  });
};

onMounted(async () => {
  floatingWindowMode.value = isInFloatingWindow();
  // 视口尺寸落进 ref：`floatingCanvasHeight` 依赖它。
  syncViewportSize();
  // 无条件绑定 —— 对同一函数引用是幂等的，不在悬浮窗里时 onFloatingResize
  // 第一行就 return，成本可忽略。见 onViewportResize 的说明。
  window.addEventListener("resize", onViewportResize);
  // 再补一层 ResizeObserver：`resize` 在 Android WebView 里不保证派发，
  // 而 `viewportSize` 是 `liveFit`（画布缩放系数）的唯一数据源。
  try {
    viewportObserver = new ResizeObserver(() => onViewportResize());
    viewportObserver.observe(document.documentElement);
  } catch (e) {
    console.warn("ResizeObserver 不可用，退回 resize 事件", e);
  }

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
    //
    // ⚠️ 但**不能**直接照做：收回时原生会重发 4 次本事件（跨度 3 秒），
    // 用户中途重新进悬浮窗时，迟到的那些会把「刚进来」误判成「已收回」。
    // 因此先回问原生一次再决定，见 confirmReturnedToApp 的说明。
    void confirmReturnedToApp();
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
      // 打上时间戳：接下来这几百毫秒里 WebView 视口还没跟上新的窗口尺寸，
      // liveFit 要先顶着用这个权威值（见其说明），否则折叠/展开时会闪一下。
      // 注意**只有**这条「原生改完窗口立刻推」的通道打戳；500ms 轮询更新
      // floatingFit 时不能打，否则会把滞后值也当成权威值。
      authoritativeAt = Date.now();
    }
    // 系数变了，内容高度的换算结果也变了，立刻按新系数重报一次
    reportFloatingHeight();
  });

  // 手机端：**立刻开始轮询**，让「我到底在不在悬浮窗里」由原生答案决定。
  //
  // 这一步是整套形态判断的地基。`onMounted` 跑的时候 `showFloatingPet()` 还没
  // 调用（goToPetMode 是先 push 后 show），所以此刻：
  //   - `isInFloatingWindow()` 是 false → 只能按桌面端布局渲染（此时 WebView
  //     确实还是整屏，桌面端那套 240×480 铺满屏幕是对的）
  //   - 原生 `status.detached` 也是 false
  // 搬移完成后原生会推 `pet-detached`，但那条事件不可靠；轮询是可靠的那条路，
  // 一旦 `detached` 变真就切到悬浮窗形态（见 enterFloatingLayout）。
  if (isAndroid()) startMetricsPolling();

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
    // resize 监听器已在 onMounted 顶部无条件绑好，这里不再重复绑
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

  // 手机端还没搬进悬浮窗的那个短暂窗口（onMounted 时 showFloatingPet 尚未调用）。
  //
  // 这一段只需要「按整屏铺满」——布局由 appStyleVars 的桌面分支给出
  // （画布 = 240 × pet.scale，在整屏 WebView 里正好铺满），而下面两步在手机上
  // 全是无意义的副作用：
  // - applyWindowLayout → set_pet_mode 只碰桌面窗口
  // - hitTestInterval → 10Hz 上报 solid region，Android 是窗口级穿透，用不上
  //
  // 早先这里没有这层拦截，页面在搬移前就开始跑桌面端的窗口逻辑；更糟的是
  // 有人（我）为了让「页面知道自己在悬浮窗里」而在挂载时抢跑成悬浮窗形态，
  // 结果宠物画布被渲染在一个**整屏** WebView 里——真机诊断实测
  // `inner=360x802 floating=true canvas=360x315@0,0`，屏幕下方空出四百多 dp。
  // 形态一律等轮询问出来的 `status.detached`，在那之前按整屏渲染。
  if (isAndroid()) return;

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
  stopSelfHeal();
  if (fitRafId !== undefined) {
    window.cancelAnimationFrame(fitRafId);
    fitRafId = undefined;
  }
  window.removeEventListener("resize", onViewportResize);
  viewportObserver?.disconnect();
  viewportObserver = undefined;
  bandObserver.disconnect();

  if (hitTestInterval !== undefined) {
    window.clearInterval(hitTestInterval);
  }
});

// 光标在桌宠窗口内就显示输入框，离开则隐藏；草稿非空（正在打字）时保持显示
const setShowChatInput = (insideWindow: boolean) => {
  showChatInput.value = insideWindow || (ChatInputRef.value?.isTyping() ?? false);
};

/**
 * 桌面端：光标进入桌宠窗口 → 显示输入框。
 *
 * 悬浮窗里**不用**这条：手机没有 hover，而 Android WebView 会把触摸
 * 合成成 mouseenter/mouseleave。悬浮窗的窗口是矩形、宠物不是，手指落在
 * 窗口边角（宠物轮廓之外、`#pet-app` 之内）就会派发一次 mouseenter/leave，
 * 把输入框的显隐交给「有没有摸到窗口角落」这种随机事件 —— 展开态被
 * 这么打断一次，用户看到的就是「按一下空白，输入框没了」。
 *
 * 悬浮窗里的输入框显隐只由 expandPet / collapsePet 决定。
 */
const handleMouseEnter = () => {
  if (floatingWindowMode.value) return;
  setShowChatInput(true);
};

const handleMouseLeave = () => {
  if (floatingWindowMode.value) return;
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

    // ★ Android 必须显式给出承载窗口的 Activity。
    //
    // Tauri 在 Android 上建窗走的是 `tao` 的
    // `AndroidContext::create_activity` —— `find_class("<包名>/<activityName>")`
    // 之后 `startActivity(Class)`。所以：
    //   · 不传 `activityName` → 找不到类，窗口建不出来，只走 `tauri://error`
    //     （表现就是「点了设置没反应」，这正是真机反馈的那个 bug）；
    //   · 传的名字要与 `com/noiq/lingchat/SettingsActivity.kt` 对上，
    //     且它已在 AndroidManifest.xml 注册。
    // 桌面端不需要这个字段（多窗口是原生能力），所以按平台给。
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
      ...(isAndroid() ? { activityName: "SettingsActivity" } : {}),
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
  //    用 replace：`/pet` 与 `/chat` 是同一界面的两种形态，push 会在 Android
  //    的 WebView 历史栈里堆出一条「chat → pet → chat → …」的来回链条，
  //    按返回键就会在两页之间反复弹（见 docs 5.4）。
  router.replace("/chat");
};
</script>

<style scoped>
/*
 * 尺寸必须走 var(--app-width/height)，不能写 100vw/100dvh：
 * ID 选择器的优先级高于 Tailwind 工具类，写成 100vw/100dvh 会把模板上的
 * `w-(--app-width) h-(--app-height)` 全部压掉，悬浮窗里页面就永远是
 * 「满视口」而不是「逻辑画布」，整体缩放随之失效。
 *
 * --pet-fit 只在悬浮窗模式下有值（= window.innerWidth / FLOATING_LOGICAL_WIDTH），
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

/*
 * 外壳层 —— **恰好是 WebView 视口**。
 *
 * `#app` 已经铺满整个视觉视口（`position: fixed` + `100dvw/100dvh`），
 * 本层 `inset: 0` 套在它里面，所以本层的尺寸**就是** WebView 的实际尺寸：
 * 不经过任何计算、不依赖任何原生上报、也不依赖 `--pet-fit`。
 *
 * 于是「宠物外面那一圈透明」在结构上不可能出现 —— 外面这一层永远是窗口
 * 本身；里层逻辑画布算大了被这里裁掉，算小了才会露白，而它的渲染宽
 * `= 210 × (视口宽 / 210) ≡ 视口宽` 是恒等式（见 liveFit）。
 *
 * 另：`#pet-app` 里的 ✕ 按钮用 `absolute top-1 right-1` 定位，依赖
 * `#pet-app` 的 `position: relative` —— 那一层没有变，位置不受影响。
 */
#pet-shell {
  position: absolute;
  inset: 0;
  overflow: hidden;
}
</style>
