<template>
  <div class="min-w-0" :style="itemStyle">
    <div
      class="flex h-11 w-full overflow-hidden rounded-xl border transition-colors duration-150"
      :class="[
        variantClass,
        interactiveClass,
        item.disabled || item.loading ? 'cursor-not-allowed opacity-50' : '',
      ]"
    >
      <button
        type="button"
        class="flex h-full min-w-0 flex-1 items-center justify-center gap-1.5 px-2.5 py-1 text-center text-[13px] leading-tight font-medium"
        :class="item.disabled || item.loading ? 'cursor-not-allowed' : 'cursor-pointer'"
        :title="item.tooltip ?? item.label"
        :disabled="item.disabled || item.loading"
        @click="onMainClick"
      >
        <LoaderCircle v-if="item.loading" :size="15" class="shrink-0 animate-spin" />
        <component :is="item.icon" v-else-if="item.icon" :size="15" class="shrink-0" />
        <span class="line-clamp-2 break-words">{{ item.label }}</span>
      </button>

      <button
        v-if="item.menu?.length"
        ref="triggerRef"
        type="button"
        class="flex h-full w-10 shrink-0 items-center justify-center border-l border-white/20"
        :class="item.disabled || item.loading ? 'cursor-not-allowed' : 'cursor-pointer'"
        :disabled="item.disabled || item.loading"
        aria-haspopup="menu"
        :aria-expanded="open"
        :aria-label="item.label"
        @click="toggleMenu"
      >
        <ChevronDown
          :size="14"
          class="shrink-0 transition-transform duration-150"
          :class="{ 'rotate-180': open }"
        />
      </button>
    </div>

    <Teleport to="body">
      <Transition name="qab-menu">
        <div
          v-if="open"
          ref="menuRef"
          class="fixed z-9999 overflow-y-auto rounded-xl border border-white/10 bg-[#1b1b24] p-1.5 shadow-[0_8px_32px_rgba(0,0,0,0.6)]"
          role="menu"
          :style="menuStyle"
        >
          <button
            v-for="entry in item.menu"
            :key="entry.key"
            type="button"
            role="menuitem"
            class="flex min-h-11 w-full items-center justify-between gap-2 rounded-lg px-3 py-2 text-left text-[13px] transition-colors duration-150"
            :class="[
              entry.active
                ? 'bg-[#79d9ff]/15 text-[#79d9ff]'
                : 'text-white/85 hover:bg-white/10 hover:text-white active:bg-white/15',
              entry.disabled ? 'cursor-not-allowed opacity-40' : 'cursor-pointer',
            ]"
            :disabled="entry.disabled"
            @click="onMenuClick(entry)"
          >
            <span class="truncate">{{ entry.label }}</span>
            <Check v-if="entry.active" :size="14" class="shrink-0" />
          </button>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch, type CSSProperties } from "vue";
import { Check, ChevronDown, LoaderCircle } from "@lucide/vue";
import type { QuickActionItem, QuickActionMenuItem } from "./action-bar.types";

const props = defineProps<{
  item: QuickActionItem;
  stretch: boolean;
  /** 最后一行只剩一个时横跨整行 */
  fullRow?: boolean;
}>();

const emit = defineEmits<{
  (e: "action", key: string): void;
  (e: "menu-action", key: string, menuKey: string): void;
}>();

const open = ref(false);
const triggerRef = ref<HTMLButtonElement | null>(null);
const menuRef = ref<HTMLElement | null>(null);
const menuStyle = ref<CSSProperties>({});

const itemStyle = computed<CSSProperties>(() => {
  const base: CSSProperties = props.stretch ? { minWidth: 0 } : { justifySelf: "start" };
  if (props.fullRow) base.gridColumn = "1 / -1";
  return base;
});

const variantClass = computed(() => {
  switch (props.item.variant) {
    case "primary":
      return "border-transparent bg-[#79d9ff] text-white";
    case "danger":
      return "border-red-400/30 bg-red-500/20 text-red-200";
    default:
      return "border-white/15 bg-white/10 text-white/80";
  }
});

const interactiveClass = computed(() => {
  if (props.item.disabled || props.item.loading) return "";
  switch (props.item.variant) {
    case "primary":
      return "hover:bg-[#5fcdf7] active:bg-[#4bbfe8]";
    case "danger":
      return "hover:bg-red-500/30 active:bg-red-500/40";
    default:
      return "hover:bg-white/20 hover:text-white active:bg-white/25";
  }
});

const MENU_MIN_WIDTH = 208;
const MENU_ITEM_HEIGHT = 44;
const MENU_MARGIN = 8;

/** 菜单贴着箭头按钮弹出；下方放不下就向上翻，左右越界则向里夹紧。 */
function positionMenu(): void {
  const trigger = triggerRef.value;
  if (!trigger) return;
  const rect = trigger.getBoundingClientRect();
  const width = Math.min(MENU_MIN_WIDTH, window.innerWidth - MENU_MARGIN * 2);
  const height = Math.min(
    (props.item.menu?.length ?? 0) * MENU_ITEM_HEIGHT + 12,
    window.innerHeight * 0.6,
  );
  const openUp = window.innerHeight - rect.bottom < height + MENU_MARGIN;
  const top = openUp ? rect.top - height - 6 : rect.bottom + 6;
  const left = Math.max(
    MENU_MARGIN,
    Math.min(rect.right - width, window.innerWidth - width - MENU_MARGIN),
  );
  menuStyle.value = {
    top: `${Math.max(MENU_MARGIN, top)}px`,
    left: `${left}px`,
    width: `${width}px`,
    maxHeight: "60dvh",
  };
}

function onDocPointerDown(event: PointerEvent): void {
  const target = event.target as Node | null;
  if (!target) return;
  if (menuRef.value?.contains(target)) return;
  if (triggerRef.value?.contains(target)) return;
  open.value = false;
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === "Escape") open.value = false;
}

function onScroll(event: Event): void {
  const target = event.target as Node | null;
  if (target && menuRef.value?.contains(target)) return;
  open.value = false;
}

function onResize(): void {
  open.value = false;
}

function addGlobalListeners(): void {
  document.addEventListener("pointerdown", onDocPointerDown, true);
  document.addEventListener("keydown", onKeydown);
  window.addEventListener("resize", onResize);
  window.addEventListener("scroll", onScroll, true);
}

function removeGlobalListeners(): void {
  document.removeEventListener("pointerdown", onDocPointerDown, true);
  document.removeEventListener("keydown", onKeydown);
  window.removeEventListener("resize", onResize);
  window.removeEventListener("scroll", onScroll, true);
}

function toggleMenu(): void {
  if (props.item.disabled || props.item.loading) return;
  if (open.value) {
    open.value = false;
    return;
  }
  positionMenu();
  open.value = true;
}

function onMainClick(): void {
  if (props.item.disabled || props.item.loading) return;
  open.value = false;
  emit("action", props.item.key);
}

function onMenuClick(entry: QuickActionMenuItem): void {
  if (entry.disabled) return;
  open.value = false;
  emit("menu-action", props.item.key, entry.key);
}

watch(open, (value) => {
  if (value) addGlobalListeners();
  else removeGlobalListeners();
});

onBeforeUnmount(removeGlobalListeners);
</script>

<style scoped>
.qab-menu-enter-active,
.qab-menu-leave-active {
  transition:
    opacity 0.15s ease,
    transform 0.15s ease;
}
.qab-menu-enter-from,
.qab-menu-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
