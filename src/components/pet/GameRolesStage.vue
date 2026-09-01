<template>
  <div class="group relative flex h-full w-full items-center justify-center">
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
        class="absolute top-1 -left-3.5 z-40 flex h-8 w-8 translate-y-2 items-center justify-center
          rounded-full border border-white/10 bg-neutral-950/60 text-white opacity-0
          shadow-[0_4px_12px_rgba(0,0,0,0.3)] backdrop-blur-xl transition-all duration-300
          group-hover:translate-y-0 group-hover:opacity-100 hover:scale-110 hover:bg-cyan-500/80
          hover:text-white"
        @click.stop="handleOpenSettings"
      >
        <Settings :size="16" />
      </button>

      <!-- 自动按钮 -->
      <button
        type="button"
        :aria-label="$t('views.pet.stage.openAutoAria')"
        :title="$t('views.pet.stage.auto')"
        class="absolute top-10 -left-3.5 z-40 flex h-8 w-8 translate-y-2 items-center justify-center
          rounded-full border border-white/10 bg-neutral-950/60 text-white opacity-0
          shadow-[0_4px_12px_rgba(0,0,0,0.3)] backdrop-blur-xl transition-all duration-300
          group-hover:translate-y-0 group-hover:opacity-100 hover:scale-110 hover:bg-cyan-500/80
          hover:text-white"
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
        class="absolute top-19 -left-3.5 z-40 flex h-8 w-8 translate-y-2 items-center justify-center
          rounded-full border border-white/10 bg-neutral-950/60 text-white opacity-0
          shadow-[0_4px_12px_rgba(0,0,0,0.3)] backdrop-blur-xl transition-all duration-300
          group-hover:translate-y-0 group-hover:opacity-100 hover:scale-110 hover:bg-cyan-500/80
          hover:text-white"
        @click.stop="handleExitPetMode"
      >
        <LogOut :size="16" />
      </button>

      <!-- 截图按钮 -->
      <div
        class="absolute top-28 -left-3.5 z-40 translate-y-2 opacity-0 transition-all duration-300
          group-hover:translate-y-0 group-hover:opacity-100"
      >
        <button
          type="button"
          :title="titleText"
          class="flex h-8 w-8 items-center justify-center rounded-full border border-white/10
            bg-neutral-950/60 text-white shadow-[0_4px_12px_rgba(0,0,0,0.3)] backdrop-blur-xl
            transition-all duration-300 hover:scale-110 hover:bg-cyan-500/80 hover:text-white"
          :style="
            hasScreenshot
              ? { color: 'var(--accent-color)', borderColor: 'var(--accent-color)' }
              : {}
          "
          @click.stop="startScreenshot"
          @contextmenu.prevent="clearScreenshot"
        >
          <Camera :size="16" />
        </button>
      </div>

      <!-- 语音输入按钮（与桌面 GameDialog 同源：useAsrInput 共享会话） -->
      <div
        class="absolute top-37 -left-3.5 z-40 translate-y-2 opacity-0 transition-all duration-300
          group-hover:translate-y-0 group-hover:opacity-100"
      >
        <button
          type="button"
          :title="micTitle"
          :disabled="!canStartMic"
          class="flex h-8 w-8 items-center justify-center rounded-full border border-white/10
            bg-neutral-950/60 text-white shadow-[0_4px_12px_rgba(0,0,0,0.3)] backdrop-blur-xl
            transition-all duration-300 hover:scale-110 hover:bg-cyan-500/80 hover:text-white
            disabled:cursor-not-allowed disabled:opacity-40"
          :class="{
            'animate-asr-breathe !border-blue-400/50 !bg-blue-950/40 !text-blue-400':
              asrInput.phase.value === 'recording',
          }"
          :style="
            asrInput.phase.value === 'idle' && autoListenOn && !autoListenActive
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

    <audio ref="mainAudio" @ended="onAudioEnded" @error="onAudioEnded"></audio>
  </div>
</template>

<script setup lang="ts">
  import { ref, computed, watch, onMounted, onUnmounted } from "vue";
  import { useI18n } from "vue-i18n";
  import { getVoiceAudio } from "@/api/services/game-info";
  import { useGameStore } from "@/stores/modules/game";
  import { useUIStore } from "@/stores/modules/ui/ui";
  import { useSettingsStore } from "@/stores/modules/settings";
  import { useScreenshot } from "@/composables/useScreenshot";
  import { useAsrStore } from "@/stores/modules/settings/asr";
  import { useAsrInput, setVoicePlaying } from "@/composables/asr";
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
  const voiceDataUrl = ref("");
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
  // 路由切换（/chat ↔ /pet）销毁 audio 元素 → 播放被浏览器终止，ended 不触发：
  // 必须主动复位 voicePlaying，否则 ASR 第 12 项门控（TTS 播放中禁用）永久卡死
  onUnmounted(() => {
    setVoicePlaying(false);
    destroyScreenshot();
  });

  // --- 音频 ---
  watch(
    () => uiStore.currentAvatarAudio,
    async (newAudio) => {
      if (!mainAudio.value) return;

      if (newAudio === "None" || !newAudio) {
        voiceDataUrl.value = "";
        mainAudio.value.pause();
        mainAudio.value.currentTime = 0;
        setVoicePlaying(false);
        emit("audio-ended");
        return;
      }

      // 前置门控（审查 M4）：watch 触发即占位 voicePlaying——getVoiceAudio
      // 网络等待（100-500ms）与 play() 微任务延迟期间 ASR 不得启动，否则
      // TTS 已出声但 voicePlaying 未置位 → 误录 AI 自己的话。失败路径回退
      setVoicePlaying(true);
      try {
        const dataUrl = await getVoiceAudio(newAudio);
        voiceDataUrl.value = dataUrl;
        mainAudio.value.src = dataUrl;
        mainAudio.value.load();
        mainAudio.value.volume = uiStore.characterVolume / 100;
        // TTS 播放中 ASR 禁用（外放 TTS 进麦克风会误识别 AI 自己的话）
        mainAudio.value
          .play()
          .then(() => {
            emit("audio-started");
          })
          .catch((e) => {
            console.error("播放失败", e);
            setVoicePlaying(false);
            // 播放失败 = 本句无音频可播：通知 audio 结束，否则 audioFinished
            // 卡 false → AUTO 自动推进永久阻塞（与 'None' 分支同因）
            emit("audio-ended");
          });
      } catch (e) {
        console.error("获取语音文件失败:", e);
        setVoicePlaying(false);
        emit("audio-ended");
      }
    }
  );

  watch(
    () => uiStore.characterVolume,
    (v) => {
      if (mainAudio.value) mainAudio.value.volume = v / 100;
    }
  );

  const onAudioEnded = () => {
    setVoicePlaying(false);
    emit("audio-ended");
  };

  // --- 语音输入（与桌面 GameDialog 同源：useAsrInput 模块级单例共享会话） ---
  const asrInput = useAsrInput();
  const asrStore = useAsrStore();

  // 三层状态（与 GameDialog 一致）：auto_listen 模式开 + 总开关开 → mic = 功能开关；
  // 总开关关（自动模式已停）→ 退化为手动录音
  const autoListenOn = computed(() => asrStore.settings.auto_listen);
  const autoListenActive = computed(() => asrInput.autoListenActive.value);
  const micIcon = computed(() => {
    if (autoListenOn.value && asrStore.settings.voice_input_enabled) {
      return autoListenActive.value ? MicOff : Mic;
    }
    return Mic;
  });
  const micTitle = computed(() => {
    if (autoListenOn.value && asrStore.settings.voice_input_enabled) {
      return autoListenActive.value
        ? t("game.dialog.asrAutoOff") // 监听中：暂停
        : t("game.dialog.asrAutoResume"); // 已暂停：恢复
    }
    return asrInput.phase.value === "recording"
      ? t("game.dialog.recordingStop")
      : t("game.dialog.voiceInput");
  });
  // mic 按钮 enabled 条件（与 GameDialog 一致）：
  // - auto_listen 模式开 + 总开关开：功能开关可用
  // - 总开关关 → 整体禁用（总开关是语音输入的总闸，手动 mic 一并关闭；
  //   显示锁只挡 auto 触发，手动不受限）
  // recognizing（识别在飞）时手动分支禁用：点击无分支可走（审查 M5），
  // 避免"按下无反应"；功能开关分支不受影响（recognizing 中仍可暂停监听）
  const canStartMic = computed(
    () =>
      (autoListenOn.value && asrStore.settings.voice_input_enabled) ||
      asrInput.phase.value === "recording" ||
      (asrInput.phase.value !== "recognizing" && asrInput.canStartAsr({ forManual: true }))
  );
  function toggleRecording() {
    // auto_listen 模式开 + 总开关开：mic 按钮 = 切换功能开关（暂停/恢复监听），
    // 不改模式设置；总开关关 → 走手动录音分支
    if (autoListenOn.value && asrStore.settings.voice_input_enabled) {
      asrInput.toggleAutoListenFunction();
      return;
    }
    if (asrInput.phase.value === "idle") {
      void asrInput.start("button").catch(() => {
        /* 会话忙时静默忽略 */
      });
    } else if (asrInput.phase.value === "recording") {
      asrInput.stop();
    }
  }

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
