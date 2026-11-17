<template>
  <MenuItem :title="title" size="large">
    <template v-if="icon" #header>
      <component :is="icon" :size="20" />
    </template>
    <div ref="gridRef" class="grid" :style="gridStyle" role="toolbar" :aria-label="title">
      <QuickActionButton
        v-for="(item, index) in items"
        :key="item.key"
        :item="item"
        :stretch="stretch"
        :full-row="lastRowSolo && index === items.length - 1"
        @action="forwardAction"
        @menu-action="forwardMenuAction"
      />
    </div>

    <div
      ref="measureRef"
      aria-hidden="true"
      class="pointer-events-none fixed"
      style="top: -9999px; left: 0; visibility: hidden"
    >
      <div
        v-for="item in items"
        :key="item.key"
        class="inline-flex items-center gap-1.5 px-2.5 text-[13px] leading-tight font-medium whitespace-nowrap"
      >
        <span v-if="item.icon" class="size-[15px] shrink-0" />
        <span class="whitespace-nowrap">{{ item.label }}</span>
      </div>
    </div>
  </MenuItem>
</template>

<script setup lang="ts">
import {
  computed,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
  type Component,
  type CSSProperties,
} from "vue";
import MenuItem from "./Menu/MenuItem.vue";
import QuickActionButton from "./QuickActionButton.vue";
import type { QuickActionItem } from "./action-bar.types";

const props = withDefaults(
  defineProps<{
    items: QuickActionItem[];
    title?: string;
    icon?: Component;
    /** 单个按钮的最小宽度，决定一行能放几个 */
    minItemWidth?: string;
    /** 强制等分列数；不传则由容器宽度自动决定 */
    columns?: number;
    /** 按钮是否等宽填满整行，默认开启 */
    stretch?: boolean;
  }>(),
  {
    title: "",
    minItemWidth: "7rem",
    stretch: true,
  },
);

const emit = defineEmits<{
  (e: "action", key: string): void;
  (e: "menu-action", key: string, menuKey: string): void;
}>();

const measureRef = ref<HTMLElement | null>(null);
const gridRef = ref<HTMLElement | null>(null);
const measuredWidth = ref(0);
const containerWidth = ref(0);

/** 箭头按钮宽 40px，外框左右边框各 1px */
const MENU_BUTTON_WIDTH = 40;
const BORDER_WIDTH = 2;
const GRID_GAP = 8;

const effectiveMinWidth = computed(() =>
  measuredWidth.value > 0 ? `${measuredWidth.value}px` : props.minItemWidth,
);

/**
 * 在能放下的列数里从多到少挑，避开最后一行只剩一个的排法；
 * 都不合适时退回最多列数，再让最后一项跨行填满。
 */
const resolvedColumns = computed<number | null>(() => {
  const count = props.items.length;
  if (count === 0) return null;
  if (props.columns) return props.columns;
  const itemWidth = measuredWidth.value;
  if (!itemWidth || containerWidth.value <= 0) return null;
  const maxColumns = Math.max(
    1,
    Math.min(count, Math.floor((containerWidth.value + GRID_GAP) / (itemWidth + GRID_GAP))),
  );
  if (maxColumns === 1) return 1;
  for (let columns = maxColumns; columns >= 2; columns -= 1) {
    if (count % columns !== 1) return columns;
  }
  return maxColumns;
});

const lastRowSolo = computed(() => {
  const columns = resolvedColumns.value;
  const count = props.items.length;
  return !!columns && count > columns && count % columns === 1;
});

const gridStyle = computed<CSSProperties>(() => {
  const columns = resolvedColumns.value;
  return {
    display: "grid",
    gap: `${GRID_GAP}px`,
    gridTemplateColumns: columns
      ? `repeat(${columns}, minmax(0, 1fr))`
      : `repeat(auto-fit, minmax(min(${effectiveMinWidth.value}, 100%), 1fr))`,
  };
});

/** 以最长按钮的自然宽度作为统一列宽，保证所有按钮等宽且文字不被截断 */
function measureItemWidth(): void {
  const root = measureRef.value;
  if (!root) return;
  let max = 0;
  Array.from(root.children).forEach((child, index) => {
    const extra = props.items[index]?.menu?.length ? MENU_BUTTON_WIDTH : 0;
    max = Math.max(max, child.getBoundingClientRect().width + extra + BORDER_WIDTH);
  });
  measuredWidth.value = Math.ceil(max);
}

let frameId = 0;
let resizeObserver: ResizeObserver | null = null;

function scheduleMeasure(): void {
  cancelAnimationFrame(frameId);
  frameId = requestAnimationFrame(measureItemWidth);
}

onMounted(() => {
  if (gridRef.value) {
    resizeObserver = new ResizeObserver((entries) => {
      const width = entries[0]?.contentRect.width ?? 0;
      if (Math.abs(width - containerWidth.value) > 0.5) containerWidth.value = width;
    });
    resizeObserver.observe(gridRef.value);
  }
  scheduleMeasure();
  void document.fonts.ready.then(scheduleMeasure);
  window.addEventListener("resize", scheduleMeasure);
});

watch(() => props.items, scheduleMeasure, { deep: true });

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  window.removeEventListener("resize", scheduleMeasure);
  cancelAnimationFrame(frameId);
});

function forwardAction(key: string): void {
  emit("action", key);
}

function forwardMenuAction(key: string, menuKey: string): void {
  emit("menu-action", key, menuKey);
}
</script>
