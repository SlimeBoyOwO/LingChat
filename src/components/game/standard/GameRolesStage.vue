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
    <audio ref="mainAudio" @ended="onAudioEnded"></audio>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useGameStore } from "@/stores/modules/game";
import { useVoicePlayback } from "@/composables/role/useVoicePlayback";
import RoleAvatar from "./GameRoleAvatar.vue";
import Live2DStage from "../live2d/Live2DStage.vue";

const gameStore = useGameStore();
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
  { castScale: 1, castOffsetY: 0 },
);

const mainAudio = ref<HTMLAudioElement | null>(null);

// 语音播放管线（TTS 期间禁用 ASR、音量实时跟随）—— 与桌宠共用同一实现
const { voiceDataUrl, onAudioEnded } = useVoicePlayback({
  audioRef: mainAudio,
  onStarted: () => emit("audio-started"),
  onEnded: () => emit("audio-ended"),
});

const lightOverlayStyle = computed(() => {
  const l = gameStore.currentScene?.lighting;
  if (!l?.overlay_enabled) return undefined;
  if (l.overlay_target !== "character" && l.overlay_target !== "both") return undefined;
  const blend = l.blend_mode !== "normal" ? l.blend_mode : "overlay";
  return `background: radial-gradient(circle at ${l.light_x}% ${l.light_y}%, ${l.overlay_color1} 0%, ${l.overlay_color2} ${l.overlay_radius}%); mix-blend-mode: ${blend}; opacity: ${l.overlay_opacity}`;
});
</script>

<style scoped></style>
