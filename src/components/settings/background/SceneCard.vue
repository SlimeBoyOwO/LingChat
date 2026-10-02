<template>
  <div
    :class="[
      `group relative flex cursor-pointer flex-col overflow-hidden rounded-xl border border-white/12.5 bg-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.1),inset_0_1px_1px_rgba(255,255,255,0.1)] backdrop-blur-[20px] backdrop-saturate-180 transition-all duration-300 hover:-translate-y-1 hover:scale-[1.01] hover:bg-white/15 hover:shadow-[0_12px_40px_rgba(0,0,0,0.15),inset_0_2px_2px_rgba(255,255,255,0.15)] hover:backdrop-blur-[25px] hover:backdrop-saturate-200`,
      selected
        ? `border-2! border-sky-400! shadow-[0_0_12px_rgba(56,189,248,0.5),0_0_3px_rgba(56,189,248,0.8),inset_0_0_8px_rgba(56,189,248,0.15)]`
        : '',
    ]"
    @click="$emit('select', scene)"
    @contextmenu.prevent="$emit('context-menu', scene, $event)"
  >
    <!-- 编辑按钮（右上角扳手）—— 插件场景只读，不提供编辑 -->
    <button
      v-if="!scene.source || scene.source === 'game'"
      class="absolute top-2 right-2 z-10 rounded-lg bg-black/50 p-1.5 text-white/60 opacity-0 transition-all group-hover:opacity-100 hover:bg-black/70 hover:text-white"
      @click.stop="$emit('edit', scene)"
      :title="$t('settings.background.scene.edit')"
    >
      <Wrench :size="16" />
    </button>
    <!-- 收藏置顶按钮（左上角星星） -->
    <button
      class="absolute top-2 left-2 z-10 rounded-lg bg-black/50 p-1.5 transition-all"
      @click.stop="$emit('toggle-favorite', scene)"
      :title="favored ? $t('settings.background.scene.unfav') : $t('settings.background.scene.fav')"
    >
      <Star
        :size="16"
        :class="favored ? 'fill-amber-400 text-amber-400' : 'text-white/60 hover:text-white'"
      />
    </button>
    <!-- 插件来源标签（右上角） -->
    <PluginTag
      v-if="scene.source && scene.source !== 'game'"
      :source="scene.source"
      class="absolute top-2 right-2 z-10"
    />

    <!-- 背景预览 -->
    <div
      class="relative flex-1 overflow-hidden after:pointer-events-none after:absolute after:inset-0 after:bg-linear-to-b after:from-transparent after:to-black/30"
    >
      <img
        v-if="scene.background"
        :src="convertFileSrc(scene.background)"
        :alt="scene.scene_name"
        class="aspect-video h-full w-full object-cover transition-transform duration-300 group-hover:scale-[1.03]"
      />
      <div
        v-else
        class="flex aspect-video h-full w-full items-center justify-center bg-black/40 text-white/20"
      >
        <Image :size="48" />
      </div>
    </div>

    <!-- 信息栏 -->
    <div
      class="relative z-2 flex flex-col gap-1 border-t border-white/20 bg-white/15 px-4 py-3 backdrop-blur-[10px]"
    >
      <span class="truncate font-medium text-white/90 drop-shadow-md">
        {{ scene.scene_name }}
      </span>
      <span v-if="scene.scene_description" class="line-clamp-2 text-xs text-white/50">{{
        scene.scene_description
      }}</span>
      <span v-else class="text-xs text-yellow-400/60 italic">{{
        $t("settings.background.scene.noDescription")
      }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { convertFileSrc } from "@tauri-apps/api/core";
import { Image, Star, Wrench } from "lucide-vue-next";
import PluginTag from "@/components/ui/PluginTag.vue";
import type { SceneInfo } from "@/api/services/scene";

defineProps<{
  scene: SceneInfo;
  /** 是否为当前激活的场景 */
  selected: boolean;
  /** 是否已收藏（收藏场景排在前面） */
  favored: boolean;
}>();

defineEmits<{
  (e: "select", scene: SceneInfo): void;
  (e: "edit", scene: SceneInfo): void;
  (e: "toggle-favorite", scene: SceneInfo): void;
  (e: "context-menu", scene: SceneInfo, event: MouseEvent): void;
}>();
</script>
