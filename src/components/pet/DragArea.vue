<script setup lang="ts">
defineProps<{
  isDragging: boolean
}>()
</script>

<template>
  <div class="relative">
      <div
        v-if="isDragging"
        class="absolute inset-0 z-50 flex flex-col items-center justify-center gap-1 rounded-full pointer-events-none"
      >      
        <!-- 光圈 -->
        <div class="absolute inset-0 rounded-full border-2 border-cyan-400/50 animate-pulse transition-all"></div>
        <div class="absolute inset-0 rounded-full border border-cyan-300/30 animate-pulse" style="animation-delay: 0.3s"></div>
        <!-- 内容 -->
        <span class="relative text-xl font-bold text-cyan-300 drop-shadow-[0_0_8px_rgba(6,182,212,0.5)]">
          {{ $t('views.dragDrop') }}
        </span>
      </div>
    <slot></slot>
  </div>
</template>