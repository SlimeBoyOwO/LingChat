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

  <!-- 角色级演出覆盖层：跟随最新 Live2D/静态展示层的位置，同时保留 DLC 恐怖特效。 -->
  <div
    class="role-container-transition pointer-events-none absolute h-full w-full origin-[center_0%]"
    :style="effectsLayerStyle"
  >
    <!-- 立绘闪现覆盖层（DDLC 式崩坏一闪）：硬切无淡入淡出，盖在正常立绘上 -->
    <img
      v-if="flashAvatarUrl"
      :src="flashAvatarUrl"
      class="sprite-flash-overlay absolute h-[102%] w-full"
      :style="flashOverlayStyle"
      alt=""
      draggable="false"
    />

    <!-- 立绘噪点侵蚀覆盖层（DDLC n_rects_ghost 式）：常驻到剧本清除 -->
    <SpriteNoiseOverlay
      v-if="activeNoise"
      :key="activeNoise.seq"
      :noise="activeNoise.noise"
      :fade-in-sec="activeNoise.fadeInSec"
      :object-fit="computedObjectFit"
    />

    <div :class="bubbleClasses" :style="bubbleStyles" class="bubble"></div>
    <audio ref="bubbleAudio"></audio>
  </div>
</template>

<script setup lang="ts">
  import { ref, computed, watch, nextTick, toRefs } from "vue";
  import type { CSSProperties } from "vue";
  import { invoke } from "@tauri-apps/api/core";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { useGameStore } from "@/stores/modules/game";
  import { useUIStore } from "@/stores/modules/ui/ui";
  import { EMOTION_CONFIG, EMOTION_CONFIG_EMO } from "@/controllers/emotion/config";
  import type { GameRole } from "@/stores/modules/game/state";
  import Live2DRolePresentation from "./Live2DRolePresentation.vue";
  import StaticRolePresentation from "./StaticRolePresentation.vue";
  import TouchAreas from "./TouchAreas.vue";
  import SpriteNoiseOverlay from "./SpriteNoiseOverlay.vue";
  import "./avatar-animation.css";

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

  const activeAnimationClass = ref("normal");
  const isBubbleVisible = ref(false);
  const currentBubbleImageUrl = ref("");
  const currentBubbleClass = ref("");

  let bubbleTimeoutId: number | null = null;
  let latestEmotionId = 0;

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

  const staticLayerStyle = computed(() => ({ ...roleLayerStyle.value, zIndex: "1" }));
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

  const targetAvatarUrl = ref("");

  // --- 立绘噪点侵蚀（DDLC n_rects_ghost 式）：常驻状态，直接按 roleId 过滤即可 ---
  const activeNoise = computed(() => {
    const n = uiStore.spriteNoise;
    return n && n.roleId === role.value.roleId ? n : null;
  });

  // --- 立绘闪现（崩坏一闪）覆盖层 ---
  const flashAvatarUrl = ref("");
  const flashOverlayStyle = computed<CSSProperties>(() => ({
    objectFit: computedObjectFit.value as CSSProperties["objectFit"],
  }));
  let flashTimerId: number | null = null;
  let flashResolveId = 0;

  watch(
    () => uiStore.spriteFlash,
    async (flash) => {
      if (!flash) {
        // 剧本结束/重置时的兜底：使尚未返回的异步解析失效，并取消旧计时器。
        flashResolveId += 1;
        if (flashTimerId !== null) {
          window.clearTimeout(flashTimerId);
          flashTimerId = null;
        }
        flashAvatarUrl.value = "";
        return;
      }
      if (flash.roleId !== role.value.roleId) return;
      const currentId = ++flashResolveId;
      const r = role.value;
      const clothesName = r.clothesName === "默认" || !r.clothesName ? "default" : r.clothesName;
      const mappedEmotion = EMOTION_CONFIG_EMO[flash.emotion] || flash.emotion;

      let url = "";
      try {
        const path = await invoke<string>("get_avatar_file", {
          characterFolder: r.character_folder,
          emotion: mappedEmotion,
          clothesName,
        });
        url = convertFileSrc(path);
      } catch {
        // 角色目录没有该演出情绪的立绘文件：静默跳过这次闪现
        return;
      }
      if (currentId !== flashResolveId) return;

      // Start the authored flash timer only after the browser has decoded the
      // image; otherwise a 300ms beat can expire while its first frame loads.
      try {
        const image = new Image();
        image.src = url;
        await image.decode();
      } catch {
        // Cached/local images may report decode errors transiently; still render.
      }
      if (currentId !== flashResolveId) return;

      flashAvatarUrl.value = url;
      if (flashTimerId !== null) window.clearTimeout(flashTimerId);
      flashTimerId = window.setTimeout(() => {
        flashAvatarUrl.value = "";
        flashTimerId = null;
      }, flash.duration * 1000);
    }
  );

  let resolveAvatarId = 0;

  async function resolveAvatar() {
    const r = role.value;
    const clothesName = r.clothesName === "默认" || !r.clothesName ? "default" : r.clothesName;
    const emotion = r.emotion;
    const mappedEmotion = EMOTION_CONFIG_EMO[emotion] || "正常";

    const currentId = ++resolveAvatarId;
    try {
      const path = await invoke<string>("get_avatar_file", {
        characterFolder: r.character_folder,
        emotion: mappedEmotion,
        clothesName,
      });
      if (currentId === resolveAvatarId) {
        targetAvatarUrl.value = convertFileSrc(path);
      }
    } catch {
      // 演出专用情绪（如"崩坏"）在角色目录里可能没有对应文件，回退到"正常"再试一次
      if (mappedEmotion !== "正常") {
        try {
          const fallback = await invoke<string>("get_avatar_file", {
            characterFolder: r.character_folder,
            emotion: "正常",
            clothesName,
          });
          if (currentId === resolveAvatarId) {
            targetAvatarUrl.value = convertFileSrc(fallback);
          }
          return;
        } catch {
          // 继续走置空兜底
        }
      }
      if (currentId === resolveAvatarId) {
        targetAvatarUrl.value = "";
      }
    }
  }

  watch(
    () => [
      role.value.roleId,
      role.value.emotion,
      role.value.clothesName,
      role.value.character_folder,
    ],
    () => resolveAvatar(),
    { immediate: true }
  );

  // 监听表情，配合子组件的加载状态播放特效
  watch(
    () => role.value.emotion,
    async (newEmotion) => {
      const currentId = ++latestEmotionId;

      // 1. 等待异步头像路径解析完成
      await resolveAvatar();

      // 2. 等待 Vue 更新 DOM 并传递给子组件
      await nextTick();

      // 3. 等待子组件的图片加载 Promise 结束
      if (presentationRef.value) {
        await presentationRef.value.waitForLoad();
      }

      // 检查是否仍然是最新的表情更新
      if (currentId !== latestEmotionId) return;

      const config = EMOTION_CONFIG[newEmotion];
      if (!config) return;

      if (config.animation && config.animation !== "none") {
        activeAnimationClass.value = config.animation;
      }

      if (config.bubbleImage && config.bubbleImage !== "none") {
        const version = Date.now();
        currentBubbleImageUrl.value = `${config.bubbleImage}?t=${version}#t=0.1`;
        currentBubbleClass.value = config.bubbleClass;
        isBubbleVisible.value = false;
        nextTick(() => {
          isBubbleVisible.value = true;

          if (bubbleTimeoutId !== null) {
            window.clearTimeout(bubbleTimeoutId);
          }
          bubbleTimeoutId = window.setTimeout(() => {
            isBubbleVisible.value = false;
            bubbleTimeoutId = null;
          }, 2000);
        });
      }

      if (config.audio && config.audio !== "none") {
        playBubbleAudio(config.audio);
      }
    },
    { immediate: true }
  );

  // 播放情绪气泡音效（音量跟随「气泡音量」设置，否则恒为满音量）
  const playBubbleAudio = (src: string) => {
    if (!bubbleAudio.value) return;
    bubbleAudio.value.volume = uiStore.bubbleVolume / 100;
    bubbleAudio.value.src = src;
    bubbleAudio.value.load();
    bubbleAudio.value.play().catch((e) => console.error("气泡音效播放失败:", e));
  };

  // 气泡音量设置变化时，对已加载的音效实时生效
  watch(
    () => uiStore.bubbleVolume,
    (v) => {
      if (bubbleAudio.value) bubbleAudio.value.volume = v / 100;
    }
  );

  // 思考中反馈：气泡 + 音效（由 currentStatus 驱动，与 emotion 解耦）
  watch(
    () => gameStore.currentStatus,
    (newStatus) => {
      if (newStatus === "thinking") {
        const config = EMOTION_CONFIG["AI思考"];
        if (config && config.bubbleImage && config.bubbleImage !== "none") {
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
        if (config?.audio && config.audio !== "none") {
          playBubbleAudio(config.audio);
        }
      } else {
        // 离开思考态：隐藏思考气泡、停掉定时器
        isBubbleVisible.value = false;
        if (bubbleTimeoutId !== null) {
          window.clearTimeout(bubbleTimeoutId);
          bubbleTimeoutId = null;
        }
      }
    }
  );

  const handleAnimationEnd = () => {
    if (activeAnimationClass.value !== "normal") {
      activeAnimationClass.value = "normal";
    }
  };
</script>

<style scoped>
  :deep(.touch-area) {
    pointer-events: auto;
  }

  /* 立绘闪现覆盖层：与主立绘同位同尺寸，硬切 + 高频抖动，模拟信号故障 */
  .sprite-flash-overlay {
    object-position: center bottom;
    z-index: 2;
    pointer-events: none;
    animation: sprite-flash-jitter 0.09s steps(2, end) infinite;
    filter: saturate(1.4) contrast(1.15);
  }

  @keyframes sprite-flash-jitter {
    0% {
      transform: translate(0, 0);
      opacity: 1;
    }
    50% {
      transform: translate(-6px, 2px);
      opacity: 0.85;
    }
    100% {
      transform: translate(4px, -3px);
      opacity: 1;
    }
  }
</style>
