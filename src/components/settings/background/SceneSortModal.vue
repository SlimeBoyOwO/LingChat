<template>
  <!-- 场景排序面板（浮层）：收藏区固定在上方，未收藏区在下方 -->
  <div
    v-if="show"
    class="fixed inset-0 z-[9998] flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm"
  >
    <div
      class="relative flex max-h-[85vh] w-full max-w-3xl flex-col overflow-hidden rounded-3xl border border-white/20 bg-slate-900/60 shadow-2xl backdrop-blur-2xl"
      @click.stop
    >
      <div class="flex items-center justify-between border-b border-white/10 bg-white/10 p-4">
        <h3 class="text-lg font-bold text-white">{{ $t("settings.background.sort.title") }}</h3>
        <button
          class="rounded-full p-2 text-white/50 transition-colors hover:bg-red-500/20 hover:text-white"
          @click="$emit('close')"
        >
          <svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </button>
      </div>

      <div
        ref="sortListRef"
        class="flex-1 space-y-2 overflow-y-auto p-4"
        @mousemove="onSortMouseMove($event)"
        @mouseup="onSortMouseUp"
        @mouseleave="onSortMouseUp"
      >
        <div
          v-for="(item, index) in sortItems"
          :key="item.id"
          @mousedown="onSortMouseDown($event, index)"
          class="flex cursor-grab items-center gap-3 rounded-xl border border-white/10 bg-white/5 p-2 select-none active:cursor-grabbing"
          :style="
            draggingIndex === index
              ? {
                  transform: 'scale(1.03)',
                  boxShadow: '0 8px 24px rgba(0,0,0,0.4)',
                  zIndex: 10,
                  position: 'relative',
                }
              : {}
          "
          :class="index === overIndex ? 'ring-2 ring-indigo-400' : ''"
        >
          <template
            v-if="
              index === 0 || isSortItemFavored(item) !== isSortItemFavored(sortItems[index - 1])
            "
          >
            <div
              class="row-span-1 flex w-full items-center gap-1 pb-1 text-xs font-bold tracking-widest text-white/40 uppercase"
            >
              <Star v-if="isSortItemFavored(item)" :size="12" class="text-amber-400" />
              {{
                isSortItemFavored(item)
                  ? $t("settings.background.sort.favoredZone")
                  : $t("settings.background.sort.unfavoredZone")
              }}
            </div>
          </template>
          <img
            v-if="item.background"
            :src="convertFileSrc(item.background)"
            class="pointer-events-none h-10 w-16 shrink-0 rounded-lg object-cover"
            :alt="item.scene_name"
          />
          <div
            v-else
            class="pointer-events-none flex h-10 w-16 shrink-0 items-center justify-center rounded-lg bg-black/40 text-white/20"
          >
            <Image :size="28" />
          </div>
          <Star
            v-if="isSortItemFavored(item)"
            :size="14"
            class="pointer-events-none shrink-0 text-amber-400"
          />
          <span class="pointer-events-none flex-1 truncate text-sm text-white/85">{{
            item.scene_name
          }}</span>
          <span class="pointer-events-none shrink-0 text-xs text-white/30">{{ index + 1 }}</span>
        </div>
        <div v-if="sortItems.length === 0" class="py-8 text-center text-sm text-white/40">
          {{ $t("settings.background.sort.empty") }}
        </div>
      </div>

      <div class="flex justify-end gap-2 border-t border-white/10 bg-white/5 p-3">
        <button
          class="rounded-full bg-white/10 px-4 py-1.5 text-sm font-bold text-white/70 hover:bg-white/20"
          @click="$emit('close')"
        >
          {{ $t("settings.background.sort.cancel") }}
        </button>
        <button
          class="rounded-full bg-indigo-500/80 px-4 py-1.5 text-sm font-bold text-white hover:bg-indigo-500"
          @click="$emit('save', sortItems)"
        >
          {{ $t("settings.background.sort.save") }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Image, Star } from "lucide-vue-next";
import type { SceneInfo } from "@/api/services/scene";

const props = defineProps<{
  show: boolean;
  /** 待排序的场景列表（打开时复制一份，取消则不落库） */
  items: SceneInfo[];
  /** 已收藏的场景 id，用于划分收藏区 / 未收藏区 */
  favoredIds: string[];
}>();

defineEmits<{
  (e: "close"): void;
  (e: "save", ordered: SceneInfo[]): void;
}>();

const sortItems = ref<SceneInfo[]>([]);
const sortListRef = ref<HTMLElement | null>(null);
const draggingIndex = ref(-1);
const overIndex = ref(-1);

// 每次打开都基于传入列表重新拷贝，避免上次拖拽的残留顺序被写回
watch(
  () => props.show,
  (show) => {
    if (show) sortItems.value = props.items.map((s) => ({ ...s }));
  },
  { immediate: true },
);

function isSortItemFavored(item: SceneInfo): boolean {
  return props.favoredIds.includes(item.id);
}

function onSortMouseDown(event: MouseEvent, index: number): void {
  event.preventDefault();
  draggingIndex.value = index;
  overIndex.value = index;
}

/** 自研指针拖拽：按指针位置找落点，并限制在所属分区内（收藏区不能拖到未收藏区） */
function onSortMouseMove(event: MouseEvent): void {
  if (draggingIndex.value === -1) return;
  const list = sortListRef.value;
  if (!list) return;
  const items = Array.from(list.querySelectorAll<HTMLElement>(":scope > div"));
  const draggedFavored = isSortItemFavored(sortItems.value[draggingIndex.value]!);
  let zoneBoundary = 0;
  sortItems.value.forEach((it, i) => {
    if (isSortItemFavored(it)) zoneBoundary = i + 1;
  });
  const pointerY = event.clientY;
  let target = draggingIndex.value;
  items.forEach((el, i) => {
    const rect = el.getBoundingClientRect();
    const mid = rect.top + rect.height / 2;
    if (pointerY > mid) target = i;
  });
  if (draggedFavored) {
    target = Math.max(0, Math.min(target, Math.max(0, zoneBoundary - 1)));
  } else {
    target = Math.max(zoneBoundary, Math.min(target, sortItems.value.length - 1));
  }
  if (target !== overIndex.value) {
    overIndex.value = target;
    const arr = [...sortItems.value];
    const [moved] = arr.splice(draggingIndex.value, 1);
    arr.splice(target, 0, moved!);
    sortItems.value = arr;
    draggingIndex.value = target;
  }
}

function onSortMouseUp(): void {
  draggingIndex.value = -1;
  overIndex.value = -1;
}
</script>
