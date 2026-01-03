<template>
  <div class="flex bg-white/10 backdrop-blur-[20px] backdrop-saturate-[180%] rounded-[16px] shadow-[0_4px_12px_rgba(0,0,0,0.08)] overflow-hidden transition-all duration-300 ease-[cubic-bezier(0.25,0.8,0.25,1)] border border-black/5 h-[180px] w-full hover:-translate-y-[3px] hover:shadow-[0_6px_16px_rgba(0,0,0,0.12)] md:h-[180px] max-md:h-[120px]">
    <div class="w-[180px] h-[180px] p-[10px] flex items-center justify-center bg-[#f8f9fa] border-r border-black/5 rounded-[16px] max-md:w-[100px] max-md:h-[100px]"">
      <img :src="avatar" :alt="name" class="w-[180px] h-[180px] object-contain rounded-[8px] p-[5px] shadow-[0_2px_4px_rgba(0,0,0,0.05)] max-md:w-[85px] max-md:h-[85px]" />
    </div>
    <div class="flex-1 flex flex-col p-[12px] relative">
      <h5 class="font-semibold text-white mb-[8px] font-['Segoe_UI',Tahoma,Geneva,Verdana,sans-serif] text-[16px] md:text-[18px]">{{ name }}</h5>
      <p class="text-[#f8f9fa] leading-[1.4] overflow-y-auto mb-[8px] text-[12px] h-[40px] md:text-[13px] md:h-[60px]">{{ info }}</p>
      <div class="character-actions">
        <slot name="actions"></slot>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import Button from '../../base/widget/Button.vue'
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