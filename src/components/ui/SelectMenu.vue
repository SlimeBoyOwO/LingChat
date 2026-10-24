<template>
  <div class="relative">
    <button
      ref="triggerRef"
      type="button"
      class="flex w-full cursor-pointer items-center justify-between gap-2 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-left text-[13px] text-white transition-colors duration-200 outline-none"
      :class="open ? 'border-[#79d9ff]/50' : 'hover:border-white/20'"
      @click="toggle"
    >
      <span class="truncate">{{ selectedLabel }}</span>
      <svg
        class="h-3.5 w-3.5 shrink-0 text-white/50 transition-transform duration-200"
        :class="{ 'rotate-180': open }"
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <polyline points="6 9 12 15 18 9"></polyline>
      </svg>
    </button>

    <Teleport to="body">
      <Transition name="select-menu-pop">
        <div
          v-if="open"
          ref="menuRef"
          class="fixed z-9999 overflow-y-auto rounded-xl border border-white/10 bg-[#1b1b24] p-1.5 shadow-[0_8px_32px_rgba(0,0,0,0.6)]"
          :style="menuStyle"
        >
          <div v-for="(grp, gi) in groups" :key="gi">
            <div v-if="grp.group" class="px-3 pt-1.5 pb-1 text-[11px] font-medium text-white/40">
              {{ grp.group }}
            </div>
            <button
              v-for="opt in grp.items"
              :key="opt.value"
              type="button"
              class="flex w-full items-center justify-between gap-2 rounded-lg px-3 py-1.5 text-left text-[13px] transition-colors duration-150"
              :class="[
                opt.value === modelValue ? 'bg-[#79d9ff]/15 text-[#79d9ff]' : 'text-white/85',
                opt.disabled
                  ? 'cursor-not-allowed opacity-40'
                  : 'cursor-pointer hover:bg-white/10 hover:text-white',
              ]"
              :disabled="opt.disabled"
              @click="choose(opt)"
            >
              <span class="truncate">{{ opt.label }}</span>
              <svg
                v-if="opt.value === modelValue"
                class="h-3.5 w-3.5 shrink-0"
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <polyline points="20 6 9 17 4 12"></polyline>
              </svg>
            </button>
          </div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";

interface SelectOption {
  label: string;
  value: string;
  group?: string;
  disabled?: boolean;
}

const props = defineProps<{
  modelValue: string;
  options: SelectOption[];
}>();

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const open = ref(false);
const triggerRef = ref<HTMLButtonElement>();
const menuRef = ref<HTMLElement>();
const menuStyle = ref<Record<string, string>>({});

const selectedLabel = computed(
  () => props.options.find((opt) => opt.value === props.modelValue)?.label ?? props.modelValue,
);

// 按 group 聚合：同组聚一起，标题只出现一次（保持首次出现的顺序）
const groups = computed(() => {
  const buckets: { group?: string; items: SelectOption[] }[] = [];
  for (const opt of props.options) {
    let bucket = buckets.find((b) => b.group === opt.group);
    if (!bucket) {
      bucket = { group: opt.group, items: [] };
      buckets.push(bucket);
    }
    bucket.items.push(opt);
  }
  return buckets;
});

function reposition() {
  const el = triggerRef.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  const gap = 4;
  const spaceBelow = window.innerHeight - rect.bottom - gap - 8;
  const spaceAbove = rect.top - gap - 8;
  const openUp = spaceBelow < 160 && spaceAbove > spaceBelow;
  const maxHeight = Math.max(120, Math.min(288, openUp ? spaceAbove : spaceBelow));
  menuStyle.value = {
    left: `${rect.left}px`,
    minWidth: `${rect.width}px`,
    maxHeight: `${maxHeight}px`,
    top: openUp ? "" : `${rect.bottom + gap}px`,
    bottom: openUp ? `${window.innerHeight - rect.top + gap}px` : "",
  };
}

function onDocMouseDown(e: MouseEvent) {
  const target = e.target as Node;
  if (triggerRef.value?.contains(target) || menuRef.value?.contains(target)) return;
  open.value = false;
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") open.value = false;
}

function toggle() {
  open.value = !open.value;
}

function choose(opt: SelectOption) {
  if (opt.disabled) return;
  emit("update:modelValue", opt.value);
  open.value = false;
}

watch(open, (value) => {
  if (value) {
    reposition();
    document.addEventListener("mousedown", onDocMouseDown);
    document.addEventListener("keydown", onKeydown);
    window.addEventListener("scroll", reposition, true);
    window.addEventListener("resize", reposition);
  } else {
    document.removeEventListener("mousedown", onDocMouseDown);
    document.removeEventListener("keydown", onKeydown);
    window.removeEventListener("scroll", reposition, true);
    window.removeEventListener("resize", reposition);
  }
});

onUnmounted(() => {
  document.removeEventListener("mousedown", onDocMouseDown);
  document.removeEventListener("keydown", onKeydown);
  window.removeEventListener("scroll", reposition, true);
  window.removeEventListener("resize", reposition);
});
</script>

<style scoped>
.select-menu-pop-enter-active,
.select-menu-pop-leave-active {
  transition:
    opacity 0.16s ease,
    transform 0.16s ease;
}
.select-menu-pop-enter-from,
.select-menu-pop-leave-to {
  opacity: 0;
  transform: translateY(-4px) scale(0.98);
}
</style>
