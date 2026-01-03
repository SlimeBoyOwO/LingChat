<template>
  <div
    class="flex bg-white/10 backdrop-blur-[20px] backdrop-saturate-180 rounded-2xl shadow-[0_4px_12px_rgba(0,0,0,0.08)] overflow-hidden transition-all duration-300 ease-[cubic-bezier(0.25,0.8,0.25,1)] border border-black/5 h-45 w-full hover:-translate-y-0.75 hover:shadow-[0_6px_16px_rgba(0,0,0,0.12)] md:h-45 max-md:h-30"
  >
    <div
      class="w-45 h-45 p-2.5 flex items-center justify-center bg-[#f8f9fa] border-r border-black/5 rounded-2xl max-md:w-25 max-md:h-25"
    >
      <img
        :src="avatar"
        :alt="name"
        class="w-45 h-45 object-contain rounded-lg p-1.25 shadow-[0_2px_4px_rgba(0,0,0,0.05)] max-md:w-21.25 max-md:h-21.25"
      />
    </div>
    <div class="flex-1 flex flex-col p-3 relative">
      <h5
        class="font-semibold text-white mb-2 font-['Segoe_UI',Tahoma,Geneva,Verdana,sans-serif] text-[16px] md:text-[18px]"
      >
        {{ name }}
      </h5>
      <p
        class="text-[#f8f9fa] leading-[1.4] overflow-y-auto mb-2 text-[12px] h-10 md:text-[13px] md:h-15"
      >
        {{ info }}
      </p>
      <div class="character-actions">
        <slot name="actions"></slot>
      </div>
    </div>
  </div>
</template>

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
