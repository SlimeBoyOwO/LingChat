<template>
  <div class="relative flex items-center justify-center w-full h-full group" @click="handleAvatarClick">
    <!-- 缩放与尺寸控制层 (无位移) -->
    <div class="relative transition-transform duration-300 ease-out animate-pet-scale"
      :style="{ width: frameSize + 'px', height: frameSize + 'px' }">
      <!-- 1. 右上角信息铭牌 -->
      <div
        class="absolute top-1 -right-4 z-50 flex flex-col items-start pointer-events-none opacity-0 translate-x-4 group-hover:opacity-100 group-hover:translate-x-0 transition-all duration-400 ease-out">
        <div
          class="bg-cyan-500 text-white text-[10px] font-black px-2 py-0.5 rounded-tl-md rounded-br-md italic shadow-sm tracking-wider">
          {{ role.roleName }}
        </div>
        <div class="text-cyan-700 dark:text-cyan-300 text-xs font-bold tracking-widest pl-1 drop-shadow-sm uppercase">
          {{ role.roleSubTitle }}
        </div>
      </div>

      <!-- 2. 设置按钮 -->
      <button type="button" aria-label="打开设置" title="设置"
        class="absolute top-1 -left-3.5 z-40 w-8 h-8 rounded-full bg-white/20 backdrop-blur-md border border-white/40 text-cyan-700 dark:text-white flex items-center justify-center opacity-0 translate-y-2 group-hover:opacity-100 group-hover:translate-y-0 hover:bg-cyan-500/80 hover:text-white hover:scale-110 shadow-[0_4px_12px_rgba(0,0,0,0.1)] transition-all duration-300"
        @click.stop="handleOpenSettings">
        <Settings :size="16"></Settings>
      </button>

      <!-- 2.5 自动按钮-->
      <button type="button" aria-label="打开自动对话" title="自动"
        class="absolute top-10 -left-3.5 z-40 w-8 h-8 rounded-full bg-white/20 backdrop-blur-md border border-white/40 text-cyan-700 dark:text-white flex items-center justify-center opacity-0 translate-y-2 group-hover:opacity-100 group-hover:translate-y-0 hover:bg-cyan-500/80 hover:text-white hover:scale-110 shadow-[0_4px_12px_rgba(0,0,0,0.1)] transition-all duration-300"
        :class="[{ active: uiStore.autoMode }]" @click.stop="handleSwitchAutoMode">
        <Play v-if="!uiStore.autoMode" :size="16"></Play>
        <Pause v-else :size="16"></Pause>
      </button>

      <!-- 2.75 返回主页按钮-->
      <button type="button" aria-label="返回主页" title="返回主页"
        class="absolute top-[76px] -left-3.5 z-40 w-8 h-8 rounded-full bg-white/20 backdrop-blur-md border border-white/40 text-cyan-700 dark:text-white flex items-center justify-center opacity-0 translate-y-2 group-hover:opacity-100 group-hover:translate-y-0 hover:bg-cyan-500/80 hover:text-white hover:scale-110 shadow-[0_4px_12px_rgba(0,0,0,0.1)] transition-all duration-300"
        @click.stop="handleExitPetMode">
        <LogOut :size="16"></LogOut>
      </button>

      <!-- 3. 常驻特效：现代科技感流光圆环 -->
      <div
        class="absolute inset-3 rounded-full border-[1.5px] border-cyan-400/20 animate-pulse-slow pointer-events-none">
      </div>
      <!-- 流光扫边特效环 -->
      <div
        class="absolute -inset-1 rounded-full pointer-events-none sweep-glow-ring drop-shadow-[0_0_6px_rgba(34,211,238,0.4)]">
      </div>

      <!-- 5. 核心头像框 -->
      <div
        class="relative w-full h-full rounded-full bg-white/10 dark:bg-black/10 backdrop-blur-md border-2 border-white/60 dark:border-white/20 shadow-[0_8px_32px_rgba(0,176,255,0.15)] overflow-hidden flex items-center justify-center transition-colors duration-300 z-10"
        data-tauri-drag-region>
        <!-- 下降效果的粒子系统 -->
        <BAParticles v-if="uiStore.currentBackgroundEffect === 'BA'"
          class="absolute inset-0 w-full h-full z-0 pointer-events-none" :particle-count="60" :speed="0.2" />

        <StarField v-if="uiStore.currentBackgroundEffect === 'StarField'"
          class="absolute inset-0 w-full h-full z-0 pointer-events-none" />

        <!-- 头像图片容器 -->
        <div :class="[
          'w-full h-full z-10 rounded-full overflow-hidden',
          containerClasses,
        ]" @animationend="handleAnimationEnd">
          <div class="w-full h-full origin-top" :style="avatarStyles">
            <ImageCrossFade ref="imageFadeRef" class="w-full h-full object-cover animate-breathing"
              :src="targetAvatarUrl" :style="imageStyles" position="center 0%" object-fit="cover" />
          </div>
        </div>

        <audio ref="bubbleAudio"></audio>
      </div>

      <!-- 6. 气泡表情 -->
      <div :class="[
        'absolute w-full h-full -top-[2%] -left-[2%] z-73 bg-contain bg-no-repeat pointer-events-none transition-all duration-300 origin-bottom-left',
        bubbleClasses,
      ]" :style="bubbleStyles"></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, toRefs } from "vue";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import BAParticles from "./BAParticles.vue";
import ImageCrossFade from "@/components/ui/ImageAcrossFade.vue";
import StarField from "../game/standard/particles/StarField.vue";
import type { GameRole } from "@/stores/modules/game/state";
import { EMOTION_CONFIG, EMOTION_CONFIG_EMO } from "@/controllers/emotion/config";
import { useSettingsStore } from "@/stores/modules/settings";
import { useUIStore } from "@/stores/modules/ui/ui";
import "./avatar-animation.css";
import { Play, Pause, Settings, LogOut } from "lucide-vue-next";

const props = defineProps<{ role: GameRole }>();
const { role } = toRefs(props);

const emit = defineEmits([
  "mouseenter",
  "mouseleave",
  "avatar-click",
  "open-settings",
  "switch-auto-mode",
  "exit-pet-mode",
]);
const bubbleAudio = ref<HTMLAudioElement | null>(null);
const imageFadeRef = ref<InstanceType<typeof ImageCrossFade> | null>(null);
const settingsStore = useSettingsStore();
const uiStore = useUIStore();

const activeAnimationClass = ref("normal");
const isBubbleVisible = ref(false);
const currentBubbleImageUrl = ref("");
const currentBubbleClass = ref("");

let bubbleTimeoutId: number | null = null;
let latestEmotionId = 0;

const frameSize = computed(() => {
  const scale = settingsStore.pet?.scale || 1;
  return Math.round(210 * scale);
});

const containerClasses = computed(() => ({
  [activeAnimationClass.value]: true,
  "opacity-100": role.value.show,
  "opacity-0": !role.value.show,
}));

const avatarStyles = computed(() => ({
  transform: `scale(${role.value.scale})`,
}));

const imageStyles = computed(() => ({
  top: `-10px`,
}));

const bubbleClasses = computed(() => ({
  "opacity-100": isBubbleVisible.value,
  "opacity-0": !isBubbleVisible.value,
  [currentBubbleClass.value]: isBubbleVisible.value && currentBubbleClass.value,
}));

const bubbleStyles = computed(() => ({
  backgroundImage: `url(${currentBubbleImageUrl.value})`,
}));

const handleAvatarClick = () => emit("avatar-click");
const handleOpenSettings = () => emit("open-settings");
const handleSwitchAutoMode = () => emit("switch-auto-mode");
const handleExitPetMode = () => emit("exit-pet-mode");

const handleAnimationEnd = () => {
  if (activeAnimationClass.value !== "normal") {
    activeAnimationClass.value = "normal";
  }
};

const targetAvatarUrl = ref('');
let resolveAvatarId = 0;

async function resolveAvatar() {
  const r = role.value;
  const clothesName = r.clothesName === '默认' || !r.clothesName ? 'default' : r.clothesName;
  const emotion = r.emotion;
  const mappedEmotion = EMOTION_CONFIG_EMO[emotion] || '正常';

  if (emotion === 'AI思考') {
    targetAvatarUrl.value = 'none';
    return;
  }

  const currentId = ++resolveAvatarId;
  try {
    const path = await invoke<string>('get_avatar_file', {
      characterFolder: r.character_folder,
      emotion: mappedEmotion,
      clothesName,
    });
    if (currentId === resolveAvatarId) {
      targetAvatarUrl.value = convertFileSrc(path);
    }
  } catch {
    if (currentId === resolveAvatarId) {
      targetAvatarUrl.value = '';
    }
  }
}

watch(
  () => [role.value.roleId, role.value.emotion, role.value.clothesName, role.value.character_folder],
  () => resolveAvatar(),
  { immediate: true },
);

watch(
  () => role.value.emotion,
  async (newEmotion) => {
    const currentId = ++latestEmotionId;
    await resolveAvatar();
    await nextTick();
    if (imageFadeRef.value) await imageFadeRef.value.waitForLoad();
    if (currentId !== latestEmotionId) return;

    const config = EMOTION_CONFIG[newEmotion];
    if (!config) return;

    if (config.animation && config.animation !== "none")
      activeAnimationClass.value = config.animation;

    if (config.bubbleImage && config.bubbleImage !== "none") {
      // 修复代码：移除 ?t=... 形式的 cache-buster，避免由于本地重新加载导致的疯狂闪烁
      currentBubbleImageUrl.value = config.bubbleImage;
      currentBubbleClass.value = config.bubbleClass;

      if (bubbleTimeoutId !== null) {
        window.clearTimeout(bubbleTimeoutId);
        bubbleTimeoutId = null;
      }

      if (!isBubbleVisible.value) {
        isBubbleVisible.value = true;
      }

      bubbleTimeoutId = window.setTimeout(() => {
        isBubbleVisible.value = false;
        bubbleTimeoutId = null;
      }, 2000);
    }

    if (config.audio && config.audio !== "none" && bubbleAudio.value) {
      bubbleAudio.value.src = config.audio;
      bubbleAudio.value.load();
      bubbleAudio.value.play().catch((e) => console.error("Play error:", e));
    }
  },
  { immediate: true },
);
</script>

<style scoped>
.animate-breathing {
  animation: breathing 4s ease-in-out infinite alternate;
}

.animate-pulse-slow {
  animation: pulse-slow 3s cubic-bezier(0.4, 0, 0.6, 1) infinite;
}

@keyframes breathing {
  0% {
    transform: scale(1);
  }
  100% {
    transform: scale(1.02);
  }
}

@keyframes pulse-slow {
  0%,
  100% {
    opacity: 0.3;
  }
  50% {
    opacity: 1;
  }
}

.sweep-glow-ring {
  background: conic-gradient(from 0deg,
      transparent 40%,
      rgba(34, 211, 238, 0.1) 70%,
      rgba(34, 211, 238, 0.8) 100%);
  -webkit-mask: radial-gradient(transparent 68%, #000 69%);
  mask: radial-gradient(transparent 68%, #000 69%);
  animation: spin 4s linear infinite;
}

[data-tauri-drag-region] {
  -webkit-app-region: drag;
}
</style>
