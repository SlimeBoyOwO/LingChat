<template>
  <div class="absolute h-full w-full overflow-hidden">
    <!-- 1. 所有 Live2D 角色共享一个场景级 Pixi Application -->
    <Live2DStage
      class="z-2"
      :roles="gameStore.presentRolesList"
      mode="standard"
      :active-speaker-id="gameStore.currentInteractRoleId"
      :audio-element="mainAudio"
      :voice-data-url="voiceDataUrl"
      :cast-scale="castScale"
      :cast-offset-y="castOffsetY"
    >
      <!-- 2. 每个角色保留原有静态视觉、气泡和触摸层 -->
      <RoleAvatar
        v-for="role in gameStore.presentRolesList"
        :key="role.roleId"
        :role="role"
        :cast-scale="castScale"
        :cast-offset-y="castOffsetY"
      />
    </Live2DStage>

    <!-- 3. 场景光照叠加层 -->
    <div
      v-if="lightOverlayStyle"
      class="pointer-events-none absolute inset-0 z-10"
      :style="lightOverlayStyle as any"
    ></div>

    <!-- 4. 全局主语音播放器 -->
    <audio ref="mainAudio" @ended="onAudioEnded" @error="onAudioEnded"></audio>
  </div>
</template>

<script setup lang="ts">
  import { computed, onUnmounted, ref, watch } from "vue";
  import { useGameStore } from "@/stores/modules/game";
  import { useUIStore } from "@/stores/modules/ui/ui";
  import { getVoiceAudio } from "@/api/services/game-info";
  import { setVoicePlaying } from "@/composables/asr";
  import RoleAvatar from "./GameRoleAvatar.vue";
  import Live2DStage from "../live2d/Live2DStage.vue";

  const gameStore = useGameStore();
  const uiStore = useUIStore();
  const emit = defineEmits(["audio-ended", "audio-started"]);

  /** 投屏全局缩放与偏移（仅投屏窗口传入；主窗口缺省无影响）。
    水平偏移由投屏窗口 .cast-role-layer 的 CSS translateX 整层平移，不在此处理。 */
  const props = withDefaults(
    defineProps<{
      /** 投屏全局缩放（作用于 Live2D / 立绘布局，保持贴底定位） */
      castScale?: number;
      /** 投屏全局垂直偏移（像素，正值下移；布局内夹紧，下移触底即止） */
      castOffsetY?: number;
    }>(),
    { castScale: 1, castOffsetY: 0 }
  );

  const mainAudio = ref<HTMLAudioElement | null>(null);
  const voiceDataUrl = ref("");

  const lightOverlayStyle = computed(() => {
    const l = gameStore.currentScene?.lighting;
    if (!l?.overlay_enabled) return undefined;
    if (l.overlay_target !== "character" && l.overlay_target !== "both") return undefined;
    const blend = l.blend_mode !== "normal" ? l.blend_mode : "overlay";
    return `background: radial-gradient(circle at ${l.light_x}% ${l.light_y}%, ${l.overlay_color1} 0%, ${l.overlay_color2} ${l.overlay_radius}%); mix-blend-mode: ${blend}; opacity: ${l.overlay_opacity}`;
  });

  // --- 音频逻辑 (全局) ---
  // 监听 UI Store 的音频播放指令
  watch(
    () => uiStore.currentAvatarAudio,
    async (newAudio) => {
      if (!mainAudio.value) return;

      // 如果设置为 'None'，停止当前播放。
      // 必须 emit audio-ended：上一句带音频（audio-started 已把 audioFinished 置
      // false）→ 本句无音频中止播放——不通知的话 AUTO 自动推进/融合续打会被
      // !audioFinished 永久阻塞（音频句之后跟无音频句的场景）
      if (newAudio === "None" || !newAudio) {
        voiceDataUrl.value = "";
        mainAudio.value.pause();
        mainAudio.value.currentTime = 0;
        setVoicePlaying(false);
        emit("audio-ended");
        return;
      }

      if (newAudio && newAudio !== "None") {
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

  // 暴露停止音频的方法给父组件
  const stopAudio = () => {
    if (mainAudio.value) {
      mainAudio.value.pause();
      mainAudio.value.currentTime = 0;
      setVoicePlaying(false);
    }
  };

  // 路由切换（/chat ↔ /pet）销毁 audio 元素 → 播放被浏览器终止，ended 不触发：
  // 必须主动复位 voicePlaying，否则 ASR 第 12 项门控（TTS 播放中禁用）永久卡死，
  // PTT/mic/auto 全部静默失效直到下一次 TTS 自然播完
  onUnmounted(() => {
    setVoicePlaying(false);
  });

  defineExpose({
    stopAudio,
  });
</script>

<style scoped></style>
