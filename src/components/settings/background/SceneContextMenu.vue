<template>
  <!-- 场景右键菜单：把场景的背景图片移动到子分类 -->
  <div
    v-if="visible"
    class="fixed inset-0 z-[9998]"
    @click="$emit('close')"
    @contextmenu.prevent="$emit('close')"
  ></div>
  <div
    v-if="visible && scene"
    class="fixed z-[9999] min-w-44 rounded-xl border border-white/15 bg-slate-900/95 p-1.5 shadow-2xl backdrop-blur-xl"
    :style="menuStyle"
    @click.stop
  >
    <div class="px-2.5 py-1.5 text-xs font-semibold text-white/40">
      {{ $t("settings.background.scene.moveToTitle") }}
    </div>
    <button
      class="block w-full rounded-lg px-2.5 py-1.5 text-left text-sm text-white/80 transition-colors hover:bg-white/10"
      @click="$emit('move', null)"
    >
      {{ $t("settings.background.scene.moveToRoot") }}
    </button>
    <button
      v-for="cat in categories"
      :key="'move-' + cat"
      class="block w-full rounded-lg px-2.5 py-1.5 text-left text-sm text-white/80 transition-colors hover:bg-white/10"
      @click="$emit('move', cat)"
    >
      {{ cat }}
    </button>
    <div v-if="categories.length === 0" class="px-2.5 py-1.5 text-xs text-white/30">
      {{ $t("settings.background.scene.moveNoCategory") }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import type { SceneInfo } from "@/api/services/scene";

const props = defineProps<{
  visible: boolean;
  /** 触发右键时的鼠标位置 */
  x: number;
  y: number;
  /** 菜单对应的场景 */
  scene: SceneInfo | null;
  /** 允许移动到的目标分类（不含「全部」与虚拟分类） */
  categories: string[];
}>();

defineEmits<{
  (e: "close"): void;
  (e: "move", category: string | null): void;
}>();

// 菜单定位：尽量不超出视口右/下边界
const menuStyle = computed(() => ({
  left: Math.max(0, Math.min(props.x, window.innerWidth - 190)) + "px",
  top: Math.max(0, Math.min(props.y, window.innerHeight - 260)) + "px",
}));
</script>
