<template>
  <div
    ref="stageRootRef"
    class="group relative flex shrink-0 items-center justify-center"
    :class="{ 'is-hovered': isStageHovered }"
    :style="{ width: frameSize + 'px', height: frameSize + 'px' }"
  >
    <!-- 缩放与尺寸控制层 (无位移) -->
    <div
      class="animate-pet-scale relative transition-transform duration-300 ease-out"
      :style="{ width: frameSize + 'px', height: frameSize + 'px' }"
    >
      <!-- 设置按钮：两个形态都在，差异全在 sideButtonClass 里
           （桌面端挂在头像左外侧、悬浮窗贴画布内侧；显隐时机两者相同 ——
            悬停 / 展开才浮现，收起即隐藏）。 -->
      <button
        type="button"
        :aria-label="$t('views.pet.stage.openSettingsAria')"
        :title="$t('views.pet.stage.settings')"
        class="absolute top-1 z-40 flex h-8 w-8 items-center justify-center rounded-full border border-white/10 bg-neutral-950/60 text-white shadow-[0_4px_12px_rgba(0,0,0,0.3)] backdrop-blur-xl transition-all duration-300 hover:scale-110 hover:bg-cyan-500/80 hover:text-white"
        :class="sideButtonClass"
        @click.stop="handleOpenSettings"
      >
        <Settings :size="16" />
      </button>

      <!-- 自动按钮。注意 :class 只能有一个 —— 定位类与「自动模式开启」的高亮
           类必须并进同一个数组，否则后写的那个会整体覆盖前一个。 -->
      <button
        type="button"
        :aria-label="$t('views.pet.stage.openAutoAria')"
        :title="$t('views.pet.stage.auto')"
        class="absolute top-10 z-40 flex h-8 w-8 items-center justify-center rounded-full border border-white/10 bg-neutral-950/60 text-white shadow-[0_4px_12px_rgba(0,0,0,0.3)] backdrop-blur-xl transition-all duration-300 hover:scale-110 hover:bg-cyan-500/80 hover:text-white"
        :class="[sideButtonClass, { '!border-cyan-400/50 !bg-cyan-500/80': uiStore.autoMode }]"
        @click.stop="handleSwitchAutoMode"
      >
        <Play v-if="!uiStore.autoMode" :size="16" />
        <Pause v-else :size="16" />
      </button>

      <!-- 返回主页按钮。悬浮窗里**就是**收回悬浮窗的入口 —— PetMode 不再另放
           一个自造的返回键，两个形态共用这一个（见 PetMode 模板里的说明）。 -->
      <button
        type="button"
        :aria-label="$t('views.pet.stage.backHome')"
        :title="$t('views.pet.stage.backHome')"
        class="absolute top-19 z-40 flex h-8 w-8 items-center justify-center rounded-full border border-white/10 bg-neutral-950/60 text-white shadow-[0_4px_12px_rgba(0,0,0,0.3)] backdrop-blur-xl transition-all duration-300 hover:scale-110 hover:bg-cyan-500/80 hover:text-white"
        :class="sideButtonClass"
        @click.stop="handleExitPetMode"
      >
        <LogOut :size="16" />
      </button>

      <!-- 截图按钮 -->
      <div class="absolute top-28 z-40 transition-all duration-300" :class="sideButtonClass">
        <button
          type="button"
          :title="titleText"
          class="flex h-8 w-8 items-center justify-center rounded-full border border-white/10 bg-neutral-950/60 text-white shadow-[0_4px_12px_rgba(0,0,0,0.3)] backdrop-blur-xl transition-all duration-300 hover:scale-110 hover:bg-cyan-500/80 hover:text-white"
          :style="
            hasScreenshot
              ? { color: 'var(--accent-color)', borderColor: 'var(--accent-color)' }
              : {}
          "
          @click.stop="startScreenshot()"
          @contextmenu.prevent="clearScreenshot"
        >
          <Camera :size="16" />
        </button>
      </div>

      <!-- 语音输入按钮（与桌面 GameDialog 同源：useAsrInput 共享会话） -->
      <div class="absolute top-37 z-40 transition-all duration-300" :class="sideButtonClass">
        <!-- 自动监听开着但当前已暂停时，用强调色提示"点一下可恢复"。
             原先这里写的是 !asrPhase，而 phase 只会是 idle/recording/recognizing
             （都是真值），该条件恒为 false、这段样式从未生效；改为显式判断 idle -->
        <button
          type="button"
          :title="micTitle"
          :disabled="!micEnabled"
          class="flex h-8 w-8 items-center justify-center rounded-full border border-white/10 bg-neutral-950/60 text-white shadow-[0_4px_12px_rgba(0,0,0,0.3)] backdrop-blur-xl transition-all duration-300 hover:scale-110 hover:bg-cyan-500/80 hover:text-white disabled:cursor-not-allowed disabled:opacity-40"
          :class="{
            'animate-asr-breathe !border-blue-400/50 !bg-blue-950/40 !text-blue-400':
              asrPhase === 'recording',
          }"
          :style="
            asrPhase === 'idle' && autoListenOn && !autoListenActive
              ? { color: 'var(--accent-color)', borderColor: 'var(--accent-color)' }
              : {}
          "
          @click.stop="toggleRecording"
        >
          <component :is="micIcon" :size="16" />
        </button>
      </div>

      <!-- Live2D 角色渲染（上游合并） -->
      <!-- 无框模式下不再给 host div 加 rounded-full，模型因此不再被裁成圆 -->
      <Live2DStage
        v-if="petLive2d"
        class="z-11"
        :class="{ 'rounded-full': !petFrameless }"
        :roles="singleRole ? [singleRole] : []"
        mode="pet"
        :active-speaker-id="gameStore.currentInteractRoleId"
        :audio-element="mainAudio"
        :voice-data-url="voiceDataUrl"
        :max-fps="live2dFps"
        @active-change="setLive2dActiveRoles"
        @failed-change="setLive2dFailedRoles"
      />

      <!-- 角色头像 -->
      <RoleAvatar
        v-if="singleRole"
        :key="singleRole.roleId"
        :role="singleRole"
        :live2d-active="petLive2d && live2dActiveRoleIds.has(singleRole.roleId)"
        :live2d-failed="petLive2d && live2dFailedRoleIds.has(singleRole.roleId)"
        :floating-window="floatingMode"
        @avatar-click="emit('avatar-click')"
      />
    </div>

    <audio ref="mainAudio" @ended="onAudioEnded" @error="onAudioEnded"></audio>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from "vue";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "vue-i18n";
import { useGameStore } from "@/stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useSettingsStore } from "@/stores/modules/settings";
import { useScreenshot } from "@/composables/useScreenshot";
import { useMicControl } from "@/composables/useMicControl";
import { useVoicePlayback } from "@/composables/role/useVoicePlayback";
import { prefersLive2d } from "@/types/live2d";
import { isAndroid } from "@/utils/platform";
import { isInFloatingWindow, onFloatingWindowModeChange } from "@/api/services/floating-pet";
import { AVATAR_BAND_BASE } from "./constants";
import RoleAvatar from "./GameRoleAvatar.vue";
import Live2DStage from "../game/live2d/Live2DStage.vue";
import { Play, Pause, Settings, LogOut, Camera, Mic, MicOff } from "lucide-vue-next";

const { t } = useI18n();
const gameStore = useGameStore();
const uiStore = useUIStore();
const settingsStore = useSettingsStore();

/**
 * 是否处于悬浮窗形态，由 `PetMode` 传入。
 *
 * 本组件只被 `PetMode` 使用，而形态判断（我到底在不在悬浮窗里）**只有
 * `PetMode` 知道**——它既可能是原生推来的 `pet-detached`，也可能是轮询
 * 问出来的 `status.detached`。早先这里自己监听事件 + 自己读
 * `isInFloatingWindow()`，等于把同一个判断做了三份，任一份不同步就会
 * 出现「画布按桌面端尺寸、却渲染在悬浮窗里」这类错位。
 *
 * 不传时退回自己判断，保持组件可独立使用。
 */
const props = defineProps<{
  /** 是否处于悬浮窗形态，见下方 `floatingMode`。 */
  floatingWindow?: boolean;
  /**
   * 悬浮窗是否处于**展开态**（收起态 = 只有一个头像）。
   *
   * 悬浮窗里它就是桌面端的「悬停」：手机没有鼠标，于是把「悬停才浮现」的
   * 元素（左侧按钮排、角色铭牌）改由展开态驱动 —— 时机与电脑端一一对应，
   * 只是触发源从鼠标换成了「点开」。见 `isStageHovered`。
   */
  expanded?: boolean;
}>();

const emit = defineEmits([
  "audio-ended",
  "audio-started",
  "avatar-click",
  "open-settings",
  "switch-auto-mode",
  "exit-pet-mode",
]);

const mainAudio = ref<HTMLAudioElement | null>(null);

// 语音播放管线（TTS 期间禁用 ASR、音量实时跟随）—— 与主界面共用同一实现
const { voiceDataUrl, onAudioEnded } = useVoicePlayback({
  audioRef: mainAudio,
  onStarted: () => emit("audio-started"),
  onEnded: () => emit("audio-ended"),
});

const live2dActiveRoleIds = ref(new Set<number>());
const live2dFailedRoleIds = ref(new Set<number>());

const setLive2dActiveRoles = (roleIds: number[]) => {
  live2dActiveRoleIds.value = new Set(roleIds);
};

const setLive2dFailedRoles = (roleIds: number[]) => {
  live2dFailedRoleIds.value = new Set(roleIds);
};

const singleRole = computed(() => {
  return gameStore.presentRolesList.length > 0 ? gameStore.presentRolesList[0] : null;
});

// 桌宠用 Live2D 还是静态立绘：角色设定里独立于主对话设置，缺省沿袭「有模型就用模型」。
//
// 必须同时喂给 Live2DStage 的 v-if 和 RoleAvatar 的 live2d-active/failed：
// Live2DStage 被 v-if 卸载时只会销毁模型、不会 emit activeChange（见其 onBeforeUnmount），
// 于是 live2dActiveRoleIds 会留着旧 id。只改 v-if 的话，切到静态立绘后 canvas 没了、
// 图片又被 v-show="!live2dActive" 藏住，桌宠会整个空掉。
const petLive2d = computed(() => !!singleRole.value && prefersLive2d(singleRole.value, "pet"));

// 无框桌宠（角色设定 → 桌宠）：只影响绘图，不影响加载哪些模型，
// 所以并进 Live2DStage 的 class 即可，无需像 petLive2d 那样喂 v-if / active 状态。
const petFrameless = computed(() => singleRole.value?.petFrameless === true);

/**
 * 是否运行在 Android 悬浮窗里（响应式）。
 *
 * 它**只改两件事**，其余一律与电脑端一致：
 *
 * 1. 画布尺寸固定 210（见 `frameSize`）；
 * 2. 悬停的触发源从指针换成展开态（见 `isStageHovered`），
 *    以及随之而来的定位微调（见 `sideButtonClass`）。
 *
 * 手机上的交互收敛为「点头像 = 展开/收起，双击 = 收回 App」。
 *
 * 形态以 `PetMode` 传进来的为准（见 `props.floatingWindow` 的说明）；
 * 没有传时才自己判断——那时只能读挂载瞬间的 `isInFloatingWindow()`，
 * 也就是「搬移完成前一律按非悬浮窗渲染」，这与 `PetMode` 的初始值一致。
 */
const ownFloatingMode = ref(isInFloatingWindow());
let floatingModeUnlisten: (() => void) | null = null;
const floatingMode = computed(() => props.floatingWindow ?? ownFloatingMode.value);

/**
 * 左侧那排圆形按钮（设置 / 自动 / 返回主页 / 截图 / 麦克风）的定位与显隐类。
 *
 * ## 显隐时机：两个形态**共用同一份**
 *
 * 电脑上这排按钮平时透明、悬停（`.is-hovered`）才浮现；悬浮窗里把「悬停」
 * 换成「展开态」（见 `isStageHovered`）。因此下面这段
 * `translate-y-2 opacity-0 group-[.is-hovered]:…` 在两条分支里**逐字相同**，
 * 连 300ms 的过渡都是同一份 —— 这就是「收起时机严格按电脑」。
 *
 * ## 定位：只有这里不一样
 *
 * 电脑端挂在头像框**左外侧**（`-left-3.5` = -14px），正好落在
 * `PET_WIDTH_BASE`(240) 比 `AVATAR_BAND_BASE`(210) 多出来的那 15px「呼吸边」里。
 * 悬浮窗的逻辑画布宽度**就是** 210（见 constants.ts 的说明），没有那圈余量，
 * `-left-3.5` 会落到画布外、被 `#pet-app` 的 `overflow-hidden` 整个裁掉 ——
 * 真机上根本点不到。所以悬浮窗改贴画布**内侧**（`left-1`），换算到电脑端的
 * 坐标系约等于「呼吸边内侧 1px」，观感一致。
 *
 * ## 悬浮窗还要多一条「隐藏时挡触摸」
 *
 * 悬浮窗的画布只有 210 宽，这排按钮**压在头像上**。`opacity-0` 的元素照样
 * 接收触摸，于是收起态点头像想展开、却会先命中那个看不见的按钮（比如直接
 * 打开设置）。电脑端按钮在头像外侧、不存在这个问题，所以这条只加在悬浮窗
 * 分支。
 *
 * `top-*` 两个形态共用：`top-1 / top-10 / top-19 / top-28 / top-37` 对应逻辑
 * y = 4 / 40 / 76 / 112 / 148，最下面那个按钮底边 148+32 = 180，仍在 210 高的
 * 头像带内 —— 收起态（窗口约 60dp）也不会越出窗口。
 *
 * 共享的静态类（圆底、描边、backdrop-blur、hover 放大）留在各按钮的 `class`
 * 里，这里只放「两个形态不一样」的那部分。
 */
const sideButtonClass = computed(() =>
  floatingMode.value
    ? "left-1 pointer-events-none translate-y-2 opacity-0 group-[.is-hovered]:pointer-events-auto group-[.is-hovered]:translate-y-0 group-[.is-hovered]:opacity-100"
    : "-left-3.5 translate-y-2 opacity-0 group-[.is-hovered]:translate-y-0 group-[.is-hovered]:opacity-100",
);

/**
 * 头像框边长（逻辑画布 px）。
 *
 * ## 悬浮窗里为什么不再跟随视口宽度
 *
 * 悬浮窗内是「固定逻辑画布 + 整体等比缩放」：一律按桌面端尺寸渲染，
 * 再由 PetMode 的 `transform: scale(窗口宽度 / 240)` 缩到窗口大小。
 *
 * 早先这里取「观测到的最小视口宽度」来自校准头像尺寸，本意是避免展开
 * 时头像跳变；但它同时让展开态窗口（2.75 倍宽）里空出一大片透明区，
 * 而 Android 悬浮窗没有逐像素穿透——那片空白既难看，又会吃掉下层
 * App 的触摸。改成整体缩放后，头像跟着画布一起放大，空白自然消失。
 *
 * 悬浮窗里也**不乘** `pet.scale`：缩放系数已由屏幕比例决定（见
 * `FloatingPetPlugin.EXPANDED_WIDTH_RATIO`），再乘一次就是双重缩放。
 */
const frameSize = computed(() => {
  const scale = floatingMode.value ? 1 : settingsStore.pet?.scale || 1;
  return Math.round(AVATAR_BAND_BASE * scale);
});

// --- 舞台悬停态（驱动按钮与角色铭牌的显隐）---
//
// 两套触发源，汇到同一个 `is-hovered` 类（模板里的 group-[.is-hovered]: 变体，
// 含 GameRoleAvatar 的角色铭牌）：
//   · 桌面端 = 指针悬停（下面这套判定）
//   · 悬浮窗 = 展开态（props.expanded）
//
// 桌面端为什么不能用 CSS :hover：光标离开 solid 区域后窗口会自动开启点击穿透
// （见 src-tauri/src/api/pet.rs 的 spawn_hit_test_poll），webview 从此收不到鼠标事件，
// :hover 会冻结在最后一次状态，按钮/铭牌第一次悬停后就再也隐藏不掉。
// 因此优先用 Rust 侧的全局鼠标广播 pet:cursor（每 50ms 上报窗口内逻辑坐标，
// 与 getBoundingClientRect 同坐标系，Live2D 视线也用的它）自行判定；
// 该事件只由桌面端轮询广播，没有它的环境（Linux 取坐标失败时）退回 DOM
// 指针事件——那些环境没有点击穿透，DOM 事件本来就是可靠的。
const stageRootRef = ref<HTMLElement | null>(null);
let cursorUnlisten: (() => void) | null = null;

/** 桌面端的指针悬停态，由 `pet:cursor` 广播 / DOM 指针事件写入。 */
const pointerHovered = ref(false);

/**
 * 悬停态 —— 驱动按钮排与角色铭牌的显隐（模板里的 `is-hovered`）。
 *
 * ## 桌面端
 *
 * 用 `pointerHovered`，由上面那套 `pet:cursor` / DOM 指针事件判定。
 *
 * ## 悬浮窗
 *
 * 直接取 `props.expanded`。**这就是「收起时机严格按电脑」**：
 * 电脑上光标离开 → 元素收起；悬浮窗里收起态 → 元素收起，一一对应。
 *
 * 不能沿用指针判定：Android WebView 会把触摸合成成 pointer 事件，手指落在
 * 窗口边角（宠物轮廓之外、`#pet-app` 之内）就派发一次 pointerdown/move，
 * 等于把显隐交给「有没有摸到窗口角落」这种随机事件 —— 表现就是展开后
 * 摸一下，按钮和铭牌莫名其妙地闪一下、或者卡住不消失。
 */
const isStageHovered = computed(() =>
  floatingMode.value ? props.expanded === true : pointerHovered.value,
);

const syncStageHover = (x: number, y: number) => {
  // 悬浮窗里指针判定已被 props.expanded 取代。这里提前返回而不是「照写不误」：
  // 那个值在悬浮窗里没有任何接收者，写进去只会白白触发一次响应式更新。
  if (floatingMode.value) return;
  const rect = stageRootRef.value?.getBoundingClientRect();
  if (!rect) return;
  // 光标移出窗口时上报的坐标会越界（负值/超出），该判断同时覆盖"离开窗口"
  pointerHovered.value = x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
};

// DOM 兜底：pointermove 覆盖鼠标环境，pointerdown 让触屏点按也能唤出按钮
// （触屏 pointerup 后紧跟 pointerleave，不能监听 leave）
const onDomPointer = (event: PointerEvent) => syncStageHover(event.clientX, event.clientY);
const stopDomPointerFallback = () => {
  window.removeEventListener("pointermove", onDomPointer);
  window.removeEventListener("pointerdown", onDomPointer);
};

onMounted(() => {
  window.addEventListener("pointermove", onDomPointer, { passive: true });
  window.addEventListener("pointerdown", onDomPointer, { passive: true });

  // 进出悬浮窗时切换按钮可见性（原生搬运完成后派发）
  floatingModeUnlisten = onFloatingWindowModeChange((active) => {
    // 只在没有外部传入形态时生效（见 props.floatingWindow 的说明）
    ownFloatingMode.value = active;
  });

  void listen<{ x: number; y: number }>("pet:cursor", (event) => {
    // 收到全局广播后 DOM 事件就没用了：穿透开启后它会停发，留着反而会用陈旧位置覆盖广播
    stopDomPointerFallback();
    syncStageHover(event.payload.x, event.payload.y);
  })
    .then((unlisten) => {
      cursorUnlisten = unlisten;
    })
    .catch(() => {
      // 无 Tauri 事件系统：继续用 DOM 指针事件兜底
    });
});

onUnmounted(() => {
  stopDomPointerFallback();
  cursorUnlisten?.();
  cursorUnlisten = null;
  floatingModeUnlisten?.();
  floatingModeUnlisten = null;
});

// Live2D 渲染帧率上限（0 = 不限制）：来自桌宠设置 pet.live2dFps，默认 30
const live2dFps = computed(() => settingsStore.pet?.live2dFps ?? 30);

// --- 截图 ---
const {
  hasScreenshot,
  init: initScreenshot,
  destroy: destroyScreenshot,
  start: startScreenshot,
  clear: clearScreenshot,
} = useScreenshot();

const titleText = computed(() => {
  if (isAndroid()) {
    return hasScreenshot.value
      ? t("views.pet.stage.retakePhoto")
      : t("views.pet.stage.photoOrImage");
  }
  return hasScreenshot.value
    ? t("views.pet.stage.retakeScreenshot")
    : t("views.pet.stage.screenshotAsk");
});

onMounted(() => initScreenshot());
onUnmounted(() => destroyScreenshot());

// --- 语音输入（与桌面 GameDialog 同源：useAsrInput 模块级单例共享会话） ---
const {
  phase: asrPhase,
  autoListenOn,
  autoListenActive,
  micEnabled,
  micTitle,
  micIconName,
  toggleRecording,
} = useMicControl();

// 只有图标形态是桌宠特有的：主界面把 micIconName 当字符串用，这里映射成 lucide 组件
const MIC_ICONS = { mic: Mic, "mic-off": MicOff } as const;
const micIcon = computed(() => MIC_ICONS[micIconName.value]);

// --- 按钮事件 ---
const handleOpenSettings = () => emit("open-settings");
const handleSwitchAutoMode = () => emit("switch-auto-mode");
const handleExitPetMode = () => emit("exit-pet-mode");
</script>

<style scoped>
.animate-pet-scale {
  animation: pet-scale-in 0.4s ease-out;
}

@keyframes pet-scale-in {
  0% {
    transform: scale(0.8);
    opacity: 0;
  }
  100% {
    transform: scale(1);
    opacity: 1;
  }
}
</style>
