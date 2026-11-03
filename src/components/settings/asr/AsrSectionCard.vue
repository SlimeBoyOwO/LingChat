<template>
  <section
    class="rounded-xl border border-white/10 bg-white/10 shadow-[0_8px_24px_rgba(0,0,0,0.10)]"
    :class="mobile ? 'p-4' : 'p-5'"
  >
    <div class="border-brand/60 mb-4 flex items-center justify-between gap-3 border-b pb-3">
      <div class="flex min-w-0 items-center gap-2.5">
        <component :is="icon" :size="20" class="text-brand shrink-0" />
        <h3 class="truncate font-bold text-white" :class="mobile ? 'text-sm' : 'text-base'">
          {{ title }}
        </h3>
      </div>
      <div class="flex shrink-0 items-center gap-2">
        <slot name="actions" />
      </div>
    </div>
    <slot />
  </section>
</template>

<script setup lang="ts">
import type { Component } from "vue";

import { isMobile } from "@/utils/platform";

defineProps<{
  title: string;
  icon: Component;
}>();

// 移动端判断只用于内边距与字号，不随窗口变化
const mobile = isMobile();
</script>
