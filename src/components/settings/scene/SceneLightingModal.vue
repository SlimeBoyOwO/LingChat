<script setup lang="ts">
/**
 * 场景光影编辑入口。
 *
 * 4-tab 旧面板（LightingPresetGrid / LightsManager / AtmosphereInspector /
 * MaterialInspector / LightingCanvasStage）已删除，全部由 lighting/workbench/ 取代。
 * 这一层只做 props/emits 转接，不持有任何面板逻辑。
 */
import type { LightingParams } from "@/api/services/scene";
import WorkbenchShell from "./lighting/workbench/WorkbenchShell.vue";

defineProps<{
  isOpen: boolean;
  sceneId?: string;
  sceneName?: string;
  backgroundSrc?: string | null;
  characterSrc?: string | null;
  initialLighting?: LightingParams | null;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "save", lighting: LightingParams): void;
}>();
</script>

<template>
  <WorkbenchShell
    :is-open="isOpen"
    :scene-id="sceneId"
    :scene-name="sceneName"
    :background-src="backgroundSrc"
    :character-src="characterSrc"
    :initial-lighting="initialLighting"
    @close="emit('close')"
    @save="emit('save', $event)"
  />
</template>
