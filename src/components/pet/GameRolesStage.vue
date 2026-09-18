<template>
  <div
    ref="stageRootRef"
    class="group relative flex h-full w-full items-center justify-center"
    :class="{ 'is-hovered': isStageHovered }"
  >
    <!-- 缩放与尺寸控制层 (无位移) -->
    <div
      class="animate-pet-scale relative transition-transform duration-300 ease-out"
      :style="{ width: frameSize + 'px', height: frameSize + 'px' }"
    >
      <!-- 设置按钮 -->
      <button
        type="button"
        :aria-label="$t('views.pet.stage.openSettingsAria')"
        :title="$t('views.pet.stage.settings')"
        class="absolute top-1 -left-3.5 z-40 flex h-8 w-8 translate-y-2 items-center justify-center rounded-full border border-white/10 bg-neutral-950/60 text-white opacity-0 shadow-[0_4px_12px_rgba(0,0,0,0.3)] backdrop-blur-xl transition-all duration-300 group-[.is-hovered]:translate-y-0 group-[.is-hovered]:opacity-100 hover:scale-110 hover:bg-cyan-500/80 hover:text-white"
        @click.stop="handleOpenSettings"
      >
        <Settings :size="16" />
      </button>

      <!-- 自动按钮 -->
      <button
        type="button"
        :aria-label="$t('views.pet.stage.openAutoAria')"
        :title="$t('views.pet.stage.auto')"
        class="absolute top-10 -left-3.5 z-40 flex h-8 w-8 translate-y-2 items-center justify-center rounded-full border border-white/10 bg-neutral-950/60 text-white opacity-0 shadow-[0_4px_12px_rgba(0,0,0,0.3)] backdrop-blur-xl transition-all duration-300 group-[.is-hovered]:translate-y-0 group-[.is-hovered]:opacity-100 hover:scale-110 hover:bg-cyan-500/80 hover:text-white"
        :class="{ '!border-cyan-400/50 !bg-cyan-500/80': uiStore.autoMode }"
        @click.stop="handleSwitchAutoMode"
      >
        <Play v-if="!uiStore.autoMode" :size="16" />
        <Pause v-else :size="16" />
      </button>

      <!-- 返回主页按钮 -->
      <button
        type="button"
        :aria-label="$t('views.pet.stage.backHome')"
        :title="$t('views.pet.stage.backHome')"
        class="absolute top-19 -left-3.5 z-40 flex h-8 w-8 translate-y-2 items-center justify-center rounded-full border border-white/10 bg-neutral-950/60 text-white opacity-0 shadow-[0_4px_12px_rgba(0,0,0,0.3)] backdrop-blur-xl transition-all duration-300 group-[.is-hovered]:translate-y-0 group-[.is-hovered]:opacity-100 hover:scale-110 hover:bg-cyan-500/80 hover:text-white"
        @click.stop="handleExitPetMode"
      >
        <LogOut :size="16" />
      </button>

      <!-- 截图按钮 -->
      <div
        class="absolute top-28 -left-3.5 z-40 translate-y-2 opacity-0 transition-all duration-300 group-[.is-hovered]:translate-y-0 group-[.is-hovered]:opacity-100"
      >
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
      <div
        class="absolute top-37 -left-3.5 z-40 translate-y-2 opacity-0 transition-all duration-300 group-[.is-hovered]:translate-y-0 group-[.is-hovered]:opacity-100"
      >
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
      <Live2DStage
        v-if="singleRole?.live2d"
        class="z-11 rounded-full"
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
        :live2d-active="live2dActiveRoleIds.has(singleRole.roleId)"
        :live2d-failed="live2dFailedRoleIds.has(singleRole.roleId)"
        @avatar-click="emit('avatar-click')"
      />
    </div>

    <audio ref="mainAudio" @ended="onAudioEnded"></audio>
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
import { isAndroid } from "@/utils/platform";
import RoleAvatar from "./GameRoleAvatar.vue";
import Live2DStage from "../game/live2d/Live2DStage.vue";
import { Play, Pause, Settings, LogOut, Camera, Mic, MicOff } from "lucide-vue-next";

const { t } = useI18n();
const gameStore = useGameStore();
const uiStore = useUIStore();
const settingsStore = useSettingsStore();

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

const frameSize = computed(() => {
  const scale = settingsStore.pet?.scale || 1;
  return Math.round(210 * scale);
});

// --- 舞台悬停态（驱动按钮与角色铭牌的显隐）---
// 桌面端不能用 CSS :hover：光标离开 solid 区域后窗口会自动开启点击穿透
// （见 src-tauri/src/api/pet.rs 的 spawn_hit_test_poll），webview 从此收不到鼠标事件，
// :hover 会冻结在最后一次状态，按钮/铭牌第一次悬停后就再也隐藏不掉。
// 因此优先用 Rust 侧的全局鼠标广播 pet:cursor（每 50ms 上报窗口内逻辑坐标，
// 与 getBoundingClientRect 同坐标系，Live2D 视线也用的它）自行判定；
// 该事件只由桌面端轮询广播，没有它的环境（移动端、Linux 取坐标失败时）退回 DOM
// 指针事件——那些环境没有点击穿透，DOM 事件本来就是可靠的。
// 模板对应 group-[.is-hovered]: 变体（含 GameRoleAvatar 的角色铭牌）。
const stageRootRef = ref<HTMLElement | null>(null);
const isStageHovered = ref(false);
let cursorUnlisten: (() => void) | null = null;

const syncStageHover = (x: number, y: number) => {
  const rect = stageRootRef.value?.getBoundingClientRect();
  if (!rect) return;
  // 光标移出窗口时上报的坐标会越界（负值/超出），该判断同时覆盖"离开窗口"
  isStageHovered.value = x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
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
