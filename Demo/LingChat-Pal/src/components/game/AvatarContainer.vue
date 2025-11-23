<template>
  <div class="avatar-container" @click="handleAvatarClick">
    <div class="avatar-frame" data-tauri-drag-region>
      <StarField
        ref="starfieldRef"
        :enabled="starfieldEnabled"
        :star-count="starCount"
        :scroll-speed="scrollSpeed"
        :colors="starColors"
        class="star-field"
      />
      <div
        :class="containerClasses"
        class="image-container character-animation normal"
      >
        <div :style="avatarStyles" class="avatar-img" id="qinling"></div>
      </div>

      <!-- 主音频播放器 -->
      <audio ref="avatarAudio"></audio>
      <!-- 气泡效果音播放器 -->
      <audio ref="bubbleAudio"></audio>
    </div>
    <div :class="bubbleClasses" :style="bubbleStyles" class="bubble"></div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick } from "vue";
import { API_CONFIG } from "../../core/api/config";
import StarField from "../particles/StarField.vue";
import { useGameStore } from "../../stores/modules/game";
import { useUIStore } from "../../stores/modules/ui/ui";
import { EMOTION_CONFIG, EMOTION_CONFIG_EMO } from "../../core/emotions/config";
import "./avatar-animation.css";

// 定义事件
const emit = defineEmits(["mouseenter", "mouseleave", "avatar-click"]);
const gameStore = useGameStore();
const uiStore = useUIStore();

const avatarAudio = ref<HTMLAudioElement | null>(null);
const bubbleAudio = ref<HTMLAudioElement | null>(null);

const activeAnimationClass = ref("normalx");
const loadedAvatarUrl = ref("");
const isBubbleVisible = ref(false);
const currentBubbleImageUrl = ref("");
const currentBubbleClass = ref("");

const targetAvatarUrl = computed(() => {
  const character = gameStore.character; // 获取当前角色
  const emotion = gameStore.avatar.emotion; // 获取当前表情

  const emotionConfig = EMOTION_CONFIG[emotion] || EMOTION_CONFIG["正常"];

  if (emotion === "AI思考") return "none"; // TODO: 神奇的小魔法字符串怎么你了
  if (!gameStore.script.isRunning) return `${emotionConfig.avatar}`;

  // TODO: 统一管理API
  return `/api/v1/chat/character/get_script_avatar/${character}/${EMOTION_CONFIG_EMO[emotion]}`;
});

const containerClasses = computed(() => ({
  [activeAnimationClass.value]: true,
  "avatar-visible": gameStore.avatar.show,
  "avatar-hidden": !gameStore.avatar.show,
}));

// 计算头像图片的 style
const avatarStyles = computed(() => ({
  // 使用预加载完成的图片 URL
  backgroundImage: `url(${loadedAvatarUrl.value})`,
  top: `${gameStore.avatar.offset_y}px`,
  transform: `scale(${gameStore.avatar.scale})`,
}));

// 计算气泡的 class
const bubbleClasses = computed(() => ({
  show: isBubbleVisible.value,
  [currentBubbleClass.value]: isBubbleVisible.value && currentBubbleClass.value,
}));

// 计算气泡的 style
const bubbleStyles = computed(() => ({
  backgroundImage: `url(${currentBubbleImageUrl.value})`,
}));

// 星空效果控制
const starfieldEnabled = ref(true);
const starCount = ref(120);
const scrollSpeed = ref(0.1);
const starColors = ref([
  "rgb(173, 216, 230)",
  "rgb(176, 224, 230)",
  "rgb(241, 141, 252)",
  "rgb(176, 230, 224)",
  "rgb(173, 230, 216)",
]);

// 处理头像点击事件
const handleAvatarClick = () => {
  console.log("AvatarContainer: 头像被点击");
  emit("avatar-click");
};

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
  () => {
    updateAvatarImage(targetAvatarUrl.value);
  }
);

watch(
  () => gameStore.avatar.emotion,
  (newEmotion) => {
    const config = EMOTION_CONFIG[newEmotion];
    if (!config) return;

    // a. 处理动画效果
    if (config.animation && config.animation !== "none") {
      activeAnimationClass.value = config.animation;
    }

    // b. 处理气泡效果
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

    // c. 播放音效
    if (config.audio && config.audio !== "none" && bubbleAudio.value) {
      bubbleAudio.value.src = config.audio;
      bubbleAudio.value.load();
      bubbleAudio.value.play();
    }
  },
  { immediate: true }
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
</script>

<style scoped>
/* 容器用于居中空心圆 */
.avatar-container {
  width: 100%;
  /*使容器高度居中 */
  position: relative;
  display: flex;
  justify-content: center;
  align-items: center;
  margin-top: 20px;
}

/* 空心圆样式 */
.avatar-frame {
  width: 210px; /* 圆环外径 */
  height: 210px;

  padding: 2px;
  border-radius: 50%; /* 关键：使其变为圆形 */
  background: transparent; /* 内部透明 */
  box-sizing: border-box;
  display: flex;
  justify-content: center;
  align-items: center;

  background: rgba(0, 0, 0, 0.01);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  border: 2px solid rgba(255, 255, 255, 0.125);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.1);
  transition: border-color 0.2s, box-shadow 0.2s;

  -webkit-app-region: drag;
}

.avatar-frame::before {
  content: "";

  width: 208px;
  height: 208px;

  background-color: transparent;
  background-image: conic-gradient(
    transparent,
    var(--accent-color),
    transparent 50%
  );

  border-radius: 50%;

  position: absolute;
  padding: 3px;
  z-index: -2;

  -webkit-mask: linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0);
  mask: linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0);
  -webkit-mask-composite: xor;
  mask-composite: exclude;

  animation: rotate 5s linear infinite;
}

.avatar-frame::after {
  content: "";
  position: absolute;
  width: 225px;
  height: 225px;
  border-radius: 50%;
  background: transparent;
  border: 1px groove rgba(255, 255, 255, 0.1);
  animation: rotate-reverse 20s linear infinite;
}

.star-field {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  border-radius: 50%;
  overflow: hidden;
  z-index: -1; /* 确保星空在最底层 */
}

@property --angle {
  syntax: "<angle>";
  initial-value: 0deg;
  inherits: false;
}

@keyframes rotate {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

/* 图片容器，稍小于空心圆 */
.image-container {
  width: 100%; /* 200px - 2*15px 边框 */
  height: 100%;
  border-radius: 50%;
  overflow: hidden; /* 关键：确保图片被裁剪为圆形 */
  background: transparent;
}

/* 替换 img 为 div 背景 */
.avatar-img {
  width: 100%;
  height: 100%;
  object-fit: cover; /* 关键：保持图片比例并填满容器 */
  background-size: contain;
  background-position: center center;
  background-repeat: no-repeat;
  z-index: 1;
  transition: background-image 0.2s ease-in-out;
  transform-origin: center 0%;
  animation: breathing 4s infinite; /* 呼吸动画 */
  overflow: hidden;
}
/*动画发光效果*/
@keyframes glow {
  0% {
    box-shadow: #329ea3 0px 0px 5px, #329ea3 0px 0px 10px, #329ea3 0px 0px 15px;
  }
  50% {
    box-shadow: #329ea3 0px 0px 10px, #329ea3 0px 0px 40px, #329ea3 0px 0px 60px;
  }
  100% {
    box-shadow: #329ea3 0px 0px 5px, #329ea3 0px 0px 10px, #329ea3 0px 0px 15px;
  }
}

@keyframes rotate-border {
  to {
    --angle: 360deg;
  }
}

/* 气泡固定定位样式 */
.bubble {
  position: absolute;
  background-size: contain;
  background-repeat: no-repeat;
  width: 80%;
  height: 80%;
  pointer-events: none;
  z-index: 2;

  /* 固定显示在人物右上方 */
  top: 0%;
  left: -2%;

  /* 默认隐藏 */
  opacity: 0;
  transition: opacity 0.3s;

  /* 初始缩放 */
  transform: scale(1);
}

/* 气泡显示时的动画 */
.bubble.show {
  opacity: 1;
}
</style>
