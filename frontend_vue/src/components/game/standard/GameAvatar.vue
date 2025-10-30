<template>
  <div
    :class="containerClasses"
    class="avatar-container character-animation"
    @animationend="handleAnimationEnd"
  >
    <div :style="avatarStyles" class="avatar-img" id="qinling"></div>
    <div :class="bubbleClasses" :style="bubbleStyles" class="bubble"></div>

    <!-- 主音频播放器 -->
    <audio ref="avatarAudio" @ended="onAudioEnded"></audio>
    <!-- 气泡效果音播放器 -->
    <audio ref="bubbleAudio"></audio>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick } from "vue";
import { API_CONFIG } from "../../../controllers/core/config";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";
import {
  EMOTION_CONFIG,
  EMOTION_CONFIG_EMO,
} from "../../../controllers/emotion/config"; // 假设 emotion config 在这里
import "./avatar-animation.css";

const gameStore = useGameStore();
const uiStore = useUIStore();
const emit = defineEmits(["audio-ended"]);

// --- 2. 模板引用（仅用于无法通过声明式处理的场景，如媒体控制） ---
const avatarAudio = ref<HTMLAudioElement | null>(null);
const bubbleAudio = ref<HTMLAudioElement | null>(null);

const activeAnimationClass = ref(gameStore.avatar.emotion);
const loadedAvatarUrl = ref("");
const isBubbleVisible = ref(false);
const currentBubbleImageUrl = ref("");
const currentBubbleClass = ref("");

const targetAvatarUrl = computed(() => {
  const character = gameStore.character; // 获取当前角色
  const emotion = gameStore.avatar.emotion; // 获取当前表情

  const emotionConfig = EMOTION_CONFIG[emotion] || EMOTION_CONFIG["正常"];

  if (character && character === "default") return `${emotionConfig.avatar}`;

  // TODO: 统一管理API
  return `/api/v1/chat/character/get_script_avatar/${character}/${EMOTION_CONFIG_EMO[emotion]}`;
});

// 计算容器的 class
const containerClasses = computed(() => ({
  // activeAnimationClass 是一个 ref，它会根据表情变化而更新
  [activeAnimationClass.value]: true,
}));

// 计算头像图片的 style
const avatarStyles = computed(() => ({
  // 使用预加载完成的图片 URL
  backgroundImage: `url(${loadedAvatarUrl.value})`,
  top: `${gameStore.avatar.offset}px`,
  transform: `scale(${gameStore.avatar.scale})`,
}));

// 计算气泡的 class
const bubbleClasses = computed(() => ({
  show: isBubbleVisible.value,
  [currentBubbleClass.value]: isBubbleVisible.value && currentBubbleClass.value,
}));

// 计算气泡的 style
const bubbleStyles = computed(() => ({
  left: `${gameStore.avatar.bubble_left}%`,
  top: `${gameStore.avatar.bubble_top}%`,
  backgroundImage: `url(${currentBubbleImageUrl.value})`,
}));

const updateAvatarImage = (newUrl: String) => {
  if (!newUrl || newUrl === "none") return;

  const timestamp = Date.now();
  const finalUrl = `${newUrl}?t=${timestamp}`; // 添加时间戳防止缓存

  // -- 图片预加载逻辑 --
  const img = new Image();
  img.onload = () => {
    // 预加载成功后，才更新真正用于显示的 `loadedAvatarUrl`
    loadedAvatarUrl.value = finalUrl;
  };
  img.onerror = () => {
    console.error(`加载头像失败: ${finalUrl}`);
    // 加载失败时，可以设置一个固定的备用头像
    loadedAvatarUrl.value = "/api/v1/chat/character/get_avatar/正常.png";
  };
  img.src = finalUrl;
};

watch(
  targetAvatarUrl, // 直接监听计算属性
  (newUrl) => {
    updateAvatarImage(newUrl);
  },
  { immediate: true } // 立即执行，确保组件挂载时就有初始头像
);

watch(
  () => gameStore.avatar.character_id,
  (newCharacterID) => {
    updateAvatarImage(targetAvatarUrl.value);
  }
);

watch(
  () => gameStore.avatar.emotion,
  (newEmotion) => {
    const config = EMOTION_CONFIG[newEmotion];
    if (!config) return;

    // a. 处理动画效果 (逻辑不变)
    if (config.animation && config.animation !== "none") {
      activeAnimationClass.value = config.animation;
    }

    // b. 处理气泡效果 (逻辑不变)
    if (config.bubbleImage && config.bubbleImage !== "none") {
      const version = Date.now();
      currentBubbleImageUrl.value = `${config.bubbleImage}?t=${version}#t=0.1`;
      currentBubbleClass.value = config.bubbleClass;

      isBubbleVisible.value = false;
      nextTick(() => {
        isBubbleVisible.value = true;
      });
      setTimeout(() => {
        isBubbleVisible.value = false;
      }, 2000);
    }

    // c. 播放音效 (逻辑不变)
    if (config.audio && config.audio !== "none" && bubbleAudio.value) {
      bubbleAudio.value.src = config.audio;
      bubbleAudio.value.load();
      bubbleAudio.value.play();
    }
  },
  { immediate: true } // 同样需要立即执行，以应用初始的动画和效果
);

// 监听主音频播放
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

// 监听音量控制
watch(
  () => uiStore.characterVolume,
  (newVolume) => {
    if (avatarAudio.value) {
      avatarAudio.value.volume = newVolume / 100;
    }
  }
);

watch(
  () => uiStore.bubbleVolume,
  (newVolume) => {
    if (bubbleAudio.value) {
      bubbleAudio.value.volume = newVolume / 100;
    }
  }
);

// --- 6. 事件处理方法 ---

const onAudioEnded = () => {
  emit("audio-ended");
};

// 动画结束后，恢复到默认的'normal'状态
const handleAnimationEnd = () => {
  // 避免在循环动画（如'normal'）结束时也重置
  if (activeAnimationClass.value !== "normal") {
    activeAnimationClass.value = "normal";
  }
};

// --- 7. 暴露给父组件的方法 ---
// 这个方法现在变得非常简单：它只负责改变状态，剩下的交给组件的响应式系统。
const setEmotion = (emotion: string) => {
  // 改变 store 中的状态，触发 watch 监听器
  gameStore.avatar.emotion = emotion; // 假设 store 中有这样一个 action
};

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

/* 替换 img 为 div 背景 */
.avatar-img {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 102%;
  background-size: contain;
  background-position: center center;
  background-repeat: no-repeat;
  z-index: 1;
  transition: background-image 0.2s ease-in-out;
  transform-origin: center 0%;
}
</style>
