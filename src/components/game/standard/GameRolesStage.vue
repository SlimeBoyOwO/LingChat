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

    <!-- 3. 场景光照叠加层（径向光，跟着立绘走） -->
    <div
      v-if="stageOverlay"
      class="pointer-events-none absolute inset-0 z-10"
      :style="stageOverlay as any"
    ></div>

    <!-- 4. 进阶光影层：方向光 / 冷暖分离 / 暗角，压在整块舞台之上 -->
    <LightingLayer />

    <!-- 5. 全局主语音播放器 -->
    <audio ref="mainAudio" @ended="onAudioEnded"></audio>
  </div>
</template>

<script setup lang="ts">
  import { computed, ref, watch } from "vue";
  import { useGameStore } from "@/stores/modules/game";
  import { useLightingStore } from "@/stores/modules/lighting";
  import { useUIStore } from "@/stores/modules/ui/ui";
  import { getVoiceAudio } from "@/api/services/game-info";
  import { setVoicePlaying } from "@/composables/useAsrInput";
  import RoleAvatar from "./GameRoleAvatar.vue";
  import LightingLayer from "./LightingLayer.vue";
  import Live2DStage from "../live2d/Live2DStage.vue";

  const gameStore = useGameStore();
  const uiStore = useUIStore();
  const lightingStore = useLightingStore();
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

  // 生效灯光由 lighting store 统一裁决（总开关 → 运行时覆盖 → 场景自带 → 默认光影），
  // 组件只消费换算好的 CSS，不再各自读 currentScene.lighting。
  const stageOverlay = computed(() => lightingStore.plan.stageOverlay);

  // --- 音频逻辑 (全局) ---
  // 监听 UI Store 的音频播放指令
  watch(
    () => uiStore.currentAvatarAudio,
    async (newAudio) => {
      if (!mainAudio.value) return;

      // 如果设置为 'None'，停止当前播放
      if (newAudio === "None" || !newAudio) {
        voiceDataUrl.value = "";
        mainAudio.value.pause();
        mainAudio.value.currentTime = 0;
        setVoicePlaying(false);
        return;
      }

      if (newAudio && newAudio !== "None") {
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
              setVoicePlaying(true);
              emit("audio-started");
            })
            .catch((e) => {
              console.error("播放失败", e);
              setVoicePlaying(false);
            });
        } catch (e) {
          console.error("获取语音文件失败:", e);
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

  defineExpose({
    stopAudio,
  });
</script>

<style scoped></style>
