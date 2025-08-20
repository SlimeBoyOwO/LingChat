<template>
  <div
    ref="avatarContainer"
    class="avatar-container character-animation normal"
  >
    <img
      ref="avatarImg"
      class="avatar-img"
      id="qinling"
      src="@/assets/images/avatar_error.png"
    />
    <div ref="avatarBubble" class="bubble"></div>
    <audio ref="avatarAudio"></audio>
    <audio ref="bubbleAudio"></audio>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from "vue";
import { API_CONFIG } from "../../../controllers/core/config";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";
import { EmotionController } from "../../../controllers/emotion/controller";
import "./avatar-animation.css";

const gameStore = useGameStore();
const uiStore = useUIStore();
const avatarContainer = ref<HTMLElement | null>(null);
const avatarImg = ref<HTMLImageElement | null>(null);
const avatarBubble = ref<HTMLElement | null>(null);
const avatarAudio = ref<HTMLAudioElement | null>(null);
const bubbleAudio = ref<HTMLAudioElement | null>(null);

let emotionController: EmotionController | null = null;

// 暴露方法给父组件
const setEmotion = (emotion: string, force: boolean = false) => {
  emotionController?.setEmotion(emotion, force);
};

onMounted(() => {
  // 传递 DOM 元素给 controller
  emotionController = new EmotionController({
    avatar: {
      container: avatarContainer.value!,
      img: avatarImg.value!,
      bubble: avatarBubble.value!,
    },
    bubbleAudio: bubbleAudio.value!,
  });

  // 初始化表情
  emotionController.setEmotion(gameStore.avatar.emotion);

  // 响应表情变化
  watch(
    () => gameStore.avatar.emotion,
    (newEmotion) => {
      emotionController?.setEmotion(newEmotion);
    }
  );

  // 响应音频变化
  watch(
    () => uiStore.currentAvatarAudio,
    (newAudio) => {
      if (avatarAudio.value && newAudio && newAudio !== "None") {
        avatarAudio.value.src = `${API_CONFIG.VOICE.BASE}/${newAudio}`;
        avatarAudio.value.load();
        avatarAudio.value.play();
      }
    }
  );
});

onBeforeUnmount(() => {
  emotionController?.destroy();
});

// 暴露方法给父组件
defineExpose({
  setEmotion,
});
</script>

<style>
.avatar-container {
  position: absolute;
  height: 100%;
  width: 100%;
}

img {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 102%;
  object-fit: contain;
  object-position: center;
  z-index: 1;
}
</style>
