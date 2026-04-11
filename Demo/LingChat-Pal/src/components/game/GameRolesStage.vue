<template>
  <!-- 1. 最多显示一个角色 -->
  <RoleAvatar v-if="singleRole" :key="singleRole.roleId" :role="singleRole" @avatar-click="handleAvatarClick"
    @open-settings="handleOpenSettings" @switch-auto-mode="handleSwitchAutoMode" />

  <!-- 2. 全局主语音播放器 -->
  <!-- 将语音逻辑放在父级，因为通常同一时间只有一段对话语音 -->
  <audio ref="mainAudio" @ended="onAudioEnded"></audio>
</template>

<script setup lang="ts">
import { ref, watch, computed } from "vue";
import { API_CONFIG } from "../../core/api/config";
import { useGameStore } from "../../stores/modules/game";
import { useUIStore } from "../../stores/modules/ui/ui";
import RoleAvatar from "./GameRoleAvatar.vue";

const gameStore = useGameStore();
const uiStore = useUIStore();
const emit = defineEmits([
  "audio-ended",
  "audio-started",
  "avatar-click",
  "open-settings",
  "switch-auto-mode",
]);

const mainAudio = ref<HTMLAudioElement | null>(null);

// 计算属性：获取第一个角色（最多显示一个角色）
const singleRole = computed(() => {
  return gameStore.presentRolesList.length > 0 ? gameStore.presentRolesList[0] : null;
});

// --- 音频逻辑 (全局) ---
// 监听 UI Store 的音频播放指令
watch(
  () => uiStore.currentAvatarAudio,
  (newAudio) => {
    if (mainAudio.value && newAudio && newAudio !== "None") {
      mainAudio.value.src = `${API_CONFIG.VOICE.BASE}/${newAudio}`;
      mainAudio.value.load();

      // 可以在这里判断是谁在说话，做一些特殊处理，例如让当前角色"动"一下
      // const speakerId = gameStore.currentInteractRoleId

      mainAudio.value.play().catch((e) => console.error("播放失败", e));
      emit("audio-started");
    }
  },
);

watch(
  () => uiStore.characterVolume,
  (v) => {
    if (mainAudio.value) mainAudio.value.volume = v / 100;
  },
);

const onAudioEnded = () => {
  emit("audio-ended");
};

const handleAvatarClick = () => {
  emit("avatar-click");
};

const handleOpenSettings = () => {
  emit("open-settings");
};

const handleSwitchAutoMode = () => {
  emit("switch-auto-mode");
}
</script>

<style scoped></style>
