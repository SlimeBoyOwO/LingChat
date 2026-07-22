<script setup lang="ts">
import { useFileDrop } from './useFileDrop'

const { isDragging } = useFileDrop()
</script>

<template>
  <div class="relative">
    <!-- 拖拽状态覆盖层 -->
    <Transition name="feed">
      <div
        v-if="isDragging"
        class="absolute inset-0 z-50 flex flex-col items-center justify-center gap-1 rounded-full pointer-events-none"
      >
        <!-- 脉冲光圈 -->
        <div class="absolute inset-0 rounded-full border-2 border-cyan-400/50 animate-pulse" />
        <div class="absolute inset-0 rounded-full border border-cyan-300/30 animate-pulse" style="animation-delay: 0.3s" />
        <!-- 内容 -->
        <span class="relative text-lg font-bold text-cyan-200 drop-shadow-[0_0_8px_rgba(6,182,212,0.5)]">
          松开投喂
        </span>
      </div>
    </Transition>

    <slot />
  </div>
</template>

<style scoped>
.feed-enter-active { transition: opacity 0.15s ease-out; }
.feed-leave-active { transition: opacity 0.1s ease-in; }
.feed-enter-from,
.feed-leave-to { opacity: 0; }
</style>