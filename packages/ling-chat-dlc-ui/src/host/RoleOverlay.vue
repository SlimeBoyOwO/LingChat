<template>
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
    :object-fit="objectFit"
  />
</template>
<script setup lang="ts">
import { ref, computed, watch, toRefs, onBeforeUnmount, type CSSProperties } from "vue";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { useUIStore } from "@lingchat/host/stores/modules/ui/ui";
import { useGameStore } from "@lingchat/host/stores/modules/game";
import { EMOTION_CONFIG_EMO } from "@lingchat/host/controllers/emotion/config";
import type { GameRole } from "@lingchat/host/stores/modules/game/state";
import SpriteNoiseOverlay from "../components/game/standard/SpriteNoiseOverlay.vue";
const props = defineProps<{ role: GameRole; objectFit: string }>();
const { role } = toRefs(props);
const uiStore = useUIStore(),
  gameStore = useGameStore();
// --- 立绘噪点侵蚀（DDLC n_rects_ghost 式）：常驻状态，直接按 roleId 过滤即可 ---
const activeNoise = computed(() => {
  const n = uiStore.spriteNoise;
  return n && n.roleId === role.value.roleId ? n : null;
});

// --- 立绘闪现（崩坏一闪）覆盖层 ---
const flashAvatarUrl = ref("");
const flashOverlayStyle = computed<CSSProperties>(() => ({
  objectFit: props.objectFit as CSSProperties["objectFit"],
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
        roleId: r.roleId,
        scriptName: gameStore.runningScript?.scriptName,
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
  },
);

onBeforeUnmount(() => {
  flashResolveId++;
  if (flashTimerId !== null) window.clearTimeout(flashTimerId);
});
</script>
<style scoped>
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
