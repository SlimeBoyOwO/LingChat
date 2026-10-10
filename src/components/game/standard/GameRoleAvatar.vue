<template>
  <Live2DRolePresentation
    v-if="useLive2d"
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
  >
    <!-- 触摸区域只对静态立绘生效：Live2D 的可摸范围来自模型的 touch_motions，
         与 body_part 无关，所以不渲染这层。 -->
    <template #overlay>
      <TouchAreas
        v-if="gameStore.command === 'touch'"
        :role="role"
        :src="targetAvatarUrl"
        :object-fit="computedObjectFit"
      />
    </template>
  </StaticRolePresentation>

  <!-- 原有气泡与情绪音效位于共享 Pixi 舞台上方。 -->
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
import { avatarObjectFit, useRoleAvatar } from "@/composables/role/useRoleAvatar";
import { standardAvatarStyle } from "@/utils/avatar-layout";
import { prefersLive2d } from "@/types/live2d";
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

// 主对话用 Live2D 还是静态立绘：角色设定里可选，缺省沿袭「有模型就用模型」
const useLive2d = computed(() => prefersLive2d(role.value, "standard"));

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
const computedObjectFit = computed(() => avatarObjectFit(uiStore.aspectRatio));

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
  const style: Record<string, string> = {
    ...standardAvatarStyle(
      role.value,
      { width: uiStore.viewportWidth, height: uiStore.viewportHeight },
      layoutPosition.value,
      props.castScale,
      props.castOffsetY,
    ),
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
