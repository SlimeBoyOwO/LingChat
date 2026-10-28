<template>
  <span
    class="inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/10 px-3 text-xs text-white/80"
    :class="mobile ? 'h-7' : 'h-7.5'"
  >
    <span class="h-[7px] w-[7px] shrink-0 rounded-full" :class="dotClass" />
    <span>{{ label }}</span>
  </span>
</template>

<script setup lang="ts">
import { computed } from "vue";

import { isMobile } from "@/utils/platform";

const props = defineProps<{
  label: string;
  tone: "ok" | "error" | "neutral";
}>();

const mobile = isMobile();

// 色值落在状态圆点上，标签本身保持半透明白底
const dotClass = computed(() => {
  if (props.tone === "ok") return "bg-emerald-300";
  if (props.tone === "error") return "bg-red-400";
  return "bg-gray-400";
});
</script>
