<template>
  <div
    class="flex bg-white/10 backdrop-blur-[20px] backdrop-saturate-180 rounded-2xl shadow-[0_4px_12px_rgba(0,0,0,0.08)] overflow-hidden transition-all duration-300 ease-[cubic-bezier(0.25,0.8,0.25,1)] border border-black/5 h-auto w-full hover:-translate-y-0.75 hover:shadow-[0_6px_16px_rgba(0,0,0,0.12)] min-h-[11rem]"
  >
    <div
      class="w-32 flex items-center justify-center bg-[#f8f9fa] border-r border-black/5 max-md:w-24 shrink-0 overflow-hidden"
    >
      <img
        :src="avatar"
        :alt="name"
        class="w-full h-full object-cover transition-transform duration-500 hover:scale-107"
      />
    </div>
    <div class="flex-1 flex flex-col p-4 relative overflow-hidden">
      <!-- 设置按钮 -->
      <div v-if="$slots.settings" class="absolute top-2 right-2 z-10">
        <slot name="settings"></slot>
      </div>

      <h5
        class="font-semibold text-white mb-2 text-[18px] md:text-[20px] pr-8 text-shadow"
      >
        {{ name }}
      </h5>
      <p
        class="text-[#f8f9fa] leading-[1.4] overflow-y-auto mb-2 text-[12px] h-10 md:text-[13px] md:h-15 opacity-90 whitespace-pre-wrap max-h-[80px]"
      >
        {{ info }}
      </p>
      <div class="character-actions">
        <slot name="actions"></slot>
      </div>
    </div>
  </div>
</template>

<style scoped>
.text-shadow {
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.4);
}
/* Scrollbar styling for info area */
.whitespace-pre-wrap::-webkit-scrollbar {
  width: 4px;
}
.whitespace-pre-wrap::-webkit-scrollbar-track {
  background: rgba(255, 255, 255, 0.05);
}
.whitespace-pre-wrap::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.2);
  border-radius: 2px;
}
</style>

<script setup lang="ts">
import type { Clothes } from '@/types'

interface CharacterProps {
  avatar?: string
  name?: string
  info?: string
  clothes?: Clothes[]
  selectClothes?: (clothes_name: string) => Promise<void>
  isClothesSelected?: (clothes_name: string) => boolean
}

const props = withDefaults(defineProps<CharacterProps>(), {})
</script>
