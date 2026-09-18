<template>
  <div
    class="group relative flex h-full w-full items-center justify-center"
    @click="handleAvatarClick"
  >
    <!-- 缩放与尺寸控制层 (无位移) -->
    <div class="relative h-full w-full">
      <!-- 1. 右上角信息铭牌（悬停显隐由祖先 .is-hovered 驱动，不能用 CSS :hover：
           窗口会开点击穿透，见 GameRolesStage.vue 的 isStageHovered 说明） -->
      <div
        class="pointer-events-none absolute top-1 -right-4 z-50 flex translate-x-4 flex-col items-start opacity-0 transition-all duration-400 ease-out group-[.is-hovered]:translate-x-0 group-[.is-hovered]:opacity-100"
      >
        <div
          class="rounded-tl-md rounded-br-md bg-cyan-500 px-2 py-0.5 text-[10px] font-black tracking-wider text-white italic shadow-sm"
        >
          {{ role.roleName }}
        </div>
        <div
          class="pl-1 text-xs font-bold tracking-widest text-cyan-700 uppercase drop-shadow-sm dark:text-cyan-300"
        >
          {{ role.roleSubTitle }}
        </div>
      </div>

      <!-- 3. 常驻特效：现代科技感流光圆环 -->
      <div
        class="animate-pulse-slow pointer-events-none absolute inset-3 rounded-full border-[1.5px] border-cyan-400/20"
      ></div>
      <!-- 流光扫边特效环 -->
      <div
        class="sweep-glow-ring pointer-events-none absolute -inset-1 rounded-full drop-shadow-[0_0_6px_rgba(34,211,238,0.4)]"
      ></div>

      <!-- 5. 核心头像框 -->
      <!--
        data-tauri-drag-region="false" 是刻意的：Tauri 注入的 drag.js 对「裸属性」要求
        事件目标就是标注元素本身（el === composedPath[0]），而下面的头像图片容器铺满整个框，
        事件目标永远是子元素，官方路径其实从未触发过；"false" 让 drag.js 显式跳过，避免它与
        下面的 startWindowDrag 形成双路径。CSS 选择器 [data-tauri-drag-region] 匹配任意值，
        Windows 的 -webkit-app-region: drag 保持原样。
      -->
      <div
        class="avatar-breath-frame relative z-10 flex h-full w-full items-center justify-center overflow-hidden rounded-full border-2 border-white/60 bg-white/10 shadow-[0_8px_32px_rgba(0,176,255,0.15)] backdrop-blur-md transition-colors duration-300 dark:border-white/20 dark:bg-black/10"
        data-tauri-drag-region="false"
        @mousedown="startWindowDrag"
        @dragstart.prevent
      >
        <!-- 下降效果的粒子系统 -->
        <BAParticles
          v-if="uiStore.currentBackgroundEffect === 'BA'"
          class="pointer-events-none absolute inset-0 z-0 h-full w-full"
          :particle-count="60"
          :speed="0.2"
        />

        <StarField
          v-if="uiStore.currentBackgroundEffect === 'StarField'"
          class="pointer-events-none absolute inset-0 z-0 h-full w-full"
        />

        <!-- 头像图片容器 -->
        <div
          :class="['z-10 h-full w-full overflow-hidden rounded-full', containerClasses]"
          @animationend="handleAnimationEnd"
        >
          <div class="h-full w-full origin-top" :style="avatarStyles">
            <div
              v-if="live2dFailed && !targetAvatarUrl"
              class="flex h-full w-full items-center justify-center text-xs text-white/60"
            >
              {{ $t("game.avatar.live2dUnavailable") }}
            </div>
            <ImageCrossFade
              v-show="!live2dActive"
              ref="imageFadeRef"
              class="animate-breathing h-full w-full object-cover"
              :src="targetAvatarUrl"
              :style="imageStyles"
              position="center 0%"
              object-fit="cover"
            />
          </div>
        </div>

        <audio ref="bubbleAudio"></audio>
      </div>

      <!-- 6. 气泡表情 -->
      <div
        :class="[
          `pointer-events-none absolute top-[-2%] left-[-2%] z-73 h-full w-full origin-bottom-left bg-contain bg-no-repeat transition-all duration-300`,
          bubbleClasses,
        ]"
        :style="bubbleStyles"
      ></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, toRefs } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import BAParticles from "../game/standard/particles/BAParticles.vue";
import ImageCrossFade from "@/components/ui/ImageAcrossFade.vue";
import StarField from "../game/standard/particles/StarField.vue";
import type { GameRole } from "@/stores/modules/game/state";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useRoleAvatar } from "@/composables/role/useRoleAvatar";
import "@/assets/styles/avatar-animation.css";

const props = defineProps<{ role: GameRole; live2dActive?: boolean; live2dFailed?: boolean }>();
const { role } = toRefs(props);

const emit = defineEmits(["avatar-click"]);
const bubbleAudio = ref<HTMLAudioElement | null>(null);
const imageFadeRef = ref<InstanceType<typeof ImageCrossFade> | null>(null);
const uiStore = useUIStore();

// ─── 窗口拖曳 ────────────────────────────────────────────────
// macOS 的 WKWebView 不支持 -webkit-app-region: drag，桌宠窗口因此完全拖不动。
// 这里手动接管：按下后位移超过阈值才进入原生窗口拖曳，未超过则保持为普通点击
// （头像的 click 仍会派发，"点击头像推进对话"不受影响）。
const DRAG_THRESHOLD_PX = 4;

const startWindowDrag = (e: MouseEvent) => {
  if (e.button !== 0) return;

  // 抑制文本选中与 <img> 的原生拖曳：原生 image drag 一旦启动，mousemove 就断流，
  // 阈值永远达不到，拖曳会在整个头像区域间歇性失效。preventDefault 不影响后续 click 派发。
  e.preventDefault();

  const startX = e.screenX;
  const startY = e.screenY;

  const cleanup = () => {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", cleanup);
  };

  const onMove = (moveEvent: MouseEvent) => {
    if (
      Math.abs(moveEvent.screenX - startX) < DRAG_THRESHOLD_PX &&
      Math.abs(moveEvent.screenY - startY) < DRAG_THRESHOLD_PX
    ) {
      return;
    }
    // 交给系统接管后 webview 收不到后续鼠标事件，先摘监听器再启动拖曳
    cleanup();
    void getCurrentWindow().startDragging();
  };

  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", cleanup);
};

// 头像解析 + 情绪演出（动画类 / 气泡 / 音效）—— 与主界面共用同一实现
const {
  targetAvatarUrl,
  activeAnimationClass,
  isBubbleVisible,
  currentBubbleImageUrl,
  currentBubbleClass,
  handleAnimationEnd,
} = useRoleAvatar({
  role,
  audioRef: bubbleAudio,
  waitForAvatarLoad: () => imageFadeRef.value?.waitForLoad(),
});

const containerClasses = computed(() => ({
  [activeAnimationClass.value]: true,
  "opacity-100": role.value.show,
  "opacity-0": !role.value.show,
}));

const avatarStyles = computed(() => ({
  transform: `scale(${role.value.scaleP}) translate(${role.value.offsetXP}px, ${role.value.offsetYP}px)`,
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
</script>

<style scoped>
/* 呼吸幅度：桌宠头像尺寸小，需要比主界面（1.0035）更明显才看得出。
     共享的 @keyframes breathing 读这个变量，见 src/assets/styles/avatar-animation.css */
.avatar-breath-frame {
  --avatar-breath-scale: 1.005;
}

/* 注意：这是给头像图片用的另一个呼吸动画，与共享 CSS 里的 @keyframes breathing 同名。
     Vue 的 scoped 样式会把 @keyframes 重命名，因此两者不会互相覆盖。 */
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
  background: conic-gradient(
    from 0deg,
    transparent 40%,
    rgba(34, 211, 238, 0.1) 70%,
    rgba(34, 211, 238, 0.8) 100%
  );
  -webkit-mask: radial-gradient(transparent 68%, #000 69%);
  mask: radial-gradient(transparent 68%, #000 69%);
  animation: spin 4s linear infinite;
  /* 性能：常驻旋转动画提升为独立合成层，避免每帧走主线程重绘
      （mask+gradient 光栅化结果被缓存，旋转退化为纯 GPU 纹理变换，视觉不变） */
  will-change: transform;
}

[data-tauri-drag-region] {
  -webkit-app-region: drag;
}
</style>
