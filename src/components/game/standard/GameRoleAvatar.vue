<template>
  <Live2DRolePresentation
    v-if="role.live2d"
    ref="presentationRef"
    :role-id="role.roleId"
    :src="targetAvatarUrl"
    :layer-style="staticLayerStyle"
    :animation-classes="containerClasses"
    :object-fit="computedObjectFit"
    @animation-end="handleAnimationEnd"
  />
  <StaticRolePresentation
    v-else
    ref="presentationRef"
    :src="targetAvatarUrl"
    :layer-style="staticLayerStyle"
    :animation-classes="containerClasses"
    :object-fit="computedObjectFit"
    @animation-end="handleAnimationEnd"
  />

  <!-- 原有气泡、触摸层和情绪音效位于共享 Pixi 舞台上方。 -->
  <TouchAreas v-if="gameStore.command === 'touch'" :body-parts="role.bodyPart" />
  <div
    class="role-container-transition pointer-events-none absolute h-full w-full origin-[center_0%]"
    :style="effectsLayerStyle"
  >
    <div :class="bubbleClasses" :style="bubbleStyles" class="bubble"></div>
    <audio ref="bubbleAudio"></audio>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, toRefs } from "vue";
import { useGameStore } from "@/stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";
import type { GameRole } from "@/stores/modules/game/state";
import Live2DRolePresentation from "./Live2DRolePresentation.vue";
import StaticRolePresentation from "./StaticRolePresentation.vue";
import TouchAreas from "./TouchAreas.vue";
import { useRoleAvatar } from "@/composables/role/useRoleAvatar";
import "@/assets/styles/avatar-animation.css";

const props = defineProps<{
  role: GameRole;
  /** 投屏全局缩放：乘在角色基础 scale 上（主窗口缺省为 1，无影响） */
  castScale?: number;
  /** 投屏全局垂直偏移（像素，正值下移；主窗口缺省 0）。
      水平偏移由投屏窗口 .cast-role-layer 的 CSS translateX 整层平移，不在此处理。 */
  castOffsetY?: number;
}>();

const gameStore = useGameStore();
const uiStore = useUIStore();
const { role } = toRefs(props);

const bubbleAudio = ref<HTMLAudioElement | null>(null);
const presentationRef = ref<
  InstanceType<typeof Live2DRolePresentation> | InstanceType<typeof StaticRolePresentation> | null
>(null);

// 头像解析 + 情绪演出（动画类 / 气泡 / 音效）—— 与桌宠共用同一实现
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
  waitForAvatarLoad: () => presentationRef.value?.waitForLoad(),
});

// --- 移动端适配：从 uiStore 读取视口尺寸（全局唯一 resize 监听） ---

// 窄屏适配：宽高比 1.0→0.5 区间，高度 100%→80%（rate=40）
const computedObjectFit = computed(() => {
  const ratio = uiStore.aspectRatio;
  if (ratio >= 1.0) return "contain";
  const percent = Math.max(80, 100 - (1.0 - ratio) * 40);
  return `auto ${Math.round(percent)}%`;
});

// 窄屏 Y 轴补偿：同步上述区间，0%→20% 视口高度上移（rate=40）
const narrowScreenYCompensation = computed(() => {
  const ratio = uiStore.aspectRatio;
  if (ratio >= 1.0) return 0;
  const percent = Math.min(20, (1.0 - ratio) * 40);
  return Math.round((uiStore.viewportHeight * percent) / 100);
});

const wideScreenYCompensation = computed(() => {
  const ratio = uiStore.aspectRatio;
  if (ratio < 2.0) return 0;
  const percent = Math.min(10, (ratio - 2.0) * 20);
  return Math.round((uiStore.viewportHeight * percent) / 100);
});

// --- 样式计算 ---
const layoutPosition = computed(() => {
  const allIds = gameStore.presentRoleIds;
  const myIndex = allIds.indexOf(role.value.roleId);
  const totalCount = allIds.length;
  if (myIndex === -1) return 50;
  return ((myIndex + 1) / (totalCount + 1)) * 100;
});

const lightingFilter = computed(() => {
  const c = gameStore.currentScene?.lighting?.character;
  if (!c) return undefined;
  const parts: string[] = [];
  if (c.brightness !== 1.0) parts.push(`brightness(${c.brightness})`);
  if (c.contrast !== 1.0) parts.push(`contrast(${c.contrast})`);
  if (c.saturation !== 1.0) parts.push(`saturate(${c.saturation})`);
  if (c.glow_radius > 0) parts.push(`drop-shadow(0 0 ${c.glow_radius}px ${c.glow_color})`);
  if (c.sepia > 0) parts.push(`sepia(${c.sepia})`);
  return parts.length > 0 ? parts.join(" ") : undefined;
});

const roleLayerStyle = computed(() => {
  const autoLeft = layoutPosition.value;
  // 投屏偏移折进位置（正值右移 / 下移），与 Live2D 同一套夹紧：立绘容器撑满视口、
  // 图片 bottom 锚定在容器底沿，容器底沿（top + 视口高 × 缩放）不越出窗口，
  // 避免 offsetY 下移时人物下方被窗口 overflow:hidden 截断；缩小才有下移空间。
  const manualOffset = role.value.offsetX || 0;
  const scaleTotal = (role.value.scale ?? 1) * (props.castScale ?? 1);
  const defaultTop =
    role.value.offsetY - narrowScreenYCompensation.value - wideScreenYCompensation.value;
  // 投屏垂直偏移（castOffsetY，正值下移）折进顶部位置，但只夹紧「投屏自己下移的那段」：
  // 角色自身配置的 role.offsetY 不参与夹紧，保持原语义。立绘容器撑满视口、图片 bottom
  // 锚定在容器底沿，容器底沿（top + 视口高 × 缩放）不越出窗口，下移触底即止。
  // 水平偏移由投屏窗口的 .cast-role-layer CSS translateX 整层平移（见 CastWindow.vue）。
  const downLimit = uiStore.viewportHeight * (1 - scaleTotal) - defaultTop;
  const castOffsetY = props.castOffsetY ?? 0;
  const effectiveOffsetY =
    castOffsetY > 0 ? Math.min(castOffsetY, Math.max(0, downLimit)) : castOffsetY;
  const top = defaultTop + effectiveOffsetY;

  const style: Record<string, string> = {
    left: `calc(${autoLeft}% + ${manualOffset}px)`,
    top: `${top}px`,
    transform: `translateX(-50%) scale(${scaleTotal})`,
    opacity: `${role.value.show ? 1 : 0}`,
    transition:
      "left 0.5s cubic-bezier(0.25, 0.8, 0.5, 1), top 0.3s ease, opacity 0.3s ease-in-out",
  };
  const filter = lightingFilter.value;
  if (filter) {
    style.filter = filter;
  }
  return style;
});

// --avatar-breath-scale 挂在动画元素的直接父层上（.normal 落在子级 ImageAcrossFade 上，
// 自定义属性会继承）。主界面角色大，幅度取更含蓄的 1.0035；桌宠小头像用 1.005。
const staticLayerStyle = computed(() => ({
  ...roleLayerStyle.value,
  zIndex: "1",
  "--avatar-breath-scale": "1.0035",
}));
const effectsLayerStyle = computed(() => ({ ...roleLayerStyle.value, zIndex: "2" }));

const containerClasses = computed(() => ({
  [activeAnimationClass.value]: true,
}));

const bubbleClasses = computed(() => ({
  show: isBubbleVisible.value,
  [currentBubbleClass.value]: isBubbleVisible.value && currentBubbleClass.value,
}));

const bubbleStyles = computed(() => ({
  left: `${+role.value.bubbleLeft + 5}%`,
  top: `${+role.value.bubbleTop - 5}%`,
  backgroundImage: `url(${currentBubbleImageUrl.value})`,
}));
</script>

<style scoped>
:deep(.touch-area) {
  pointer-events: auto;
}
</style>
