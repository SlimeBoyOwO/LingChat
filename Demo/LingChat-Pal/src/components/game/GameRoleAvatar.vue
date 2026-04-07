<template>
  <div class="avatar-container" @click="handleAvatarClick">
    <div class="avatar-frame" data-tauri-drag-region>
      <!-- 星空背景 -->
      <StarField
        ref="starfieldRef"
        :enabled="starfieldEnabled"
        :star-count="starCount"
        :scroll-speed="scrollSpeed"
        :colors="starColors"
        class="star-field"
      />

      <!-- 头像容器 -->
      <div
        :class="containerClasses"
        class="image-container character-animation"
        @animationend="handleAnimationEnd"
      >
        <!-- 使用新的图片淡入淡出组件替换原来的 div 纯背景图 -->
        <ImageCrossFade
          ref="imageFadeRef"
          class="avatar-img"
          :style="avatarStyles"
          :src="targetAvatarUrl"
          position="center 0%"
          object-fit="cover"
        />
      </div>
      <!-- 气泡效果音播放器 -->
      <audio ref="bubbleAudio"></audio>
    </div>

    <!-- 气泡 -->
    <div :class="bubbleClasses" :style="bubbleStyles" class="bubble"></div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, toRefs } from "vue";
import StarField from "../particles/StarField.vue";
import ImageCrossFade from "@/components/ui/ImageAcrossFade.vue"; // 确保路径正确
import type { GameRole } from "../../stores/modules/game/state";
import { EMOTION_CONFIG, EMOTION_CONFIG_EMO } from "../../core/emotions/config";
import "./avatar-animation.css";

const props = defineProps<{
  role: GameRole;
}>();

const { role } = toRefs(props);

// 定义事件
const emit = defineEmits(["mouseenter", "mouseleave", "avatar-click"]);
const bubbleAudio = ref<HTMLAudioElement | null>(null);
const imageFadeRef = ref<InstanceType<typeof ImageCrossFade> | null>(null);

const activeAnimationClass = ref("normal");
const isBubbleVisible = ref(false);
const currentBubbleImageUrl = ref("");
const currentBubbleClass = ref("");

let bubbleTimeoutId: number | null = null;
let latestEmotionId = 0;

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

const targetAvatarUrl = computed(() => {
  const r = role.value;
  const clothesName =
    r.clothesName === "默认" || !r.clothesName ? "default" : r.clothesName;
  const emotion = r.emotion;

  const mappedEmotion = EMOTION_CONFIG_EMO[emotion] || "正常";
  if (emotion === "AI思考") return "none";

  return `/api/v1/chat/character/get_avatar/${r.roleId}/${mappedEmotion}/${clothesName}`;
});

const containerClasses = computed(() => ({
  [activeAnimationClass.value]: true,
  "avatar-visible": role.value.show,
  "avatar-hidden": !role.value.show,
}));

// 计算头像图片的 style
const avatarStyles = computed(() => ({
  top: `${role.value.offsetY}px`,
  transform: `scale(${role.value.scale})`,
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

// 处理头像点击事件
const handleAvatarClick = () => {
  console.log("AvatarContainer: 头像被点击");
  emit("avatar-click");
};

// 动画结束自动恢复状态
const handleAnimationEnd = () => {
  if (activeAnimationClass.value !== "normal") {
    activeAnimationClass.value = "normal";
  }
};

// 监听表情变化，配合子组件图片淡入淡出完成再执行音效与动画
watch(
  () => role.value.emotion,
  async (newEmotion) => {
    const currentId = ++latestEmotionId;

    // 1. 等待 Vue computed(targetAvatarUrl) 更新传递给子组件
    await nextTick();

    // 2. 等待 ImageCrossFade 子组件图片加载 Promise 结束
    if (imageFadeRef.value) {
      await imageFadeRef.value.waitForLoad();
    }

    // 防止在异步加载期间表情已被多次更改导致竞态
    if (currentId !== latestEmotionId) return;

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

        // 自动隐藏气泡的清理逻辑
        if (bubbleTimeoutId !== null) {
          window.clearTimeout(bubbleTimeoutId);
        }
        bubbleTimeoutId = window.setTimeout(() => {
          isBubbleVisible.value = false;
          bubbleTimeoutId = null;
        }, 2000);
      });
    }

    // c. 播放音效
    if (config.audio && config.audio !== "none" && bubbleAudio.value) {
      bubbleAudio.value.src = config.audio;
      bubbleAudio.value.load();
      bubbleAudio.value.play();
    }
  },
  { immediate: true },
);
</script>

<style scoped>
/* 容器用于居中空心圆 */
.avatar-container {
  width: 100%;
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
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.1);
  transition:
    border-color 0.2s,
    box-shadow 0.2s;

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

  -webkit-mask:
    linear-gradient(#fff 0 0) content-box,
    linear-gradient(#fff 0 0);
  mask:
    linear-gradient(#fff 0 0) content-box,
    linear-gradient(#fff 0 0);
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
  z-index: -1;
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
  width: 100%;
  height: 100%;
  border-radius: 50%;
  overflow: hidden; /* 关键：确保图片被裁剪为圆形 */
  background: transparent;
}

/* 去除原本旧的 background 相关CSS，保留基础宽高和动画 */
.avatar-img {
  width: 100%;
  height: 100%;
  z-index: 1;
  transform-origin: center 0%;
  animation: breathing 4s infinite; /* 呼吸动画 */
  overflow: hidden;
}

/* 动画发光效果 */
@keyframes glow {
  0% {
    box-shadow:
      #329ea3 0px 0px 5px,
      #329ea3 0px 0px 10px,
      #329ea3 0px 0px 15px;
  }
  50% {
    box-shadow:
      #329ea3 0px 0px 10px,
      #329ea3 0px 0px 40px,
      #329ea3 0px 0px 60px;
  }
  100% {
    box-shadow:
      #329ea3 0px 0px 5px,
      #329ea3 0px 0px 10px,
      #329ea3 0px 0px 15px;
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
  top: 0%;
  left: -2%;
  opacity: 0;
  transition: opacity 0.3s;
  transform: scale(1);
}

.bubble.show {
  opacity: 1;
}
</style>
