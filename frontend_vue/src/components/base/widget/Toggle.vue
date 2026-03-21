<template>
  <div class="flex items-center">
    <input
      type="checkbox"
      :id="id"
      :checked="internalChecked"
      @change="handleChange"
      class="hidden"
    />
    <label
      :for="id"
      class="relative cursor-pointer text-white text-[14px] select-none inline-flex items-center"
      style="text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3)"
    >
      <span
        class="relative inline-block w-12.5 h-6.5 rounded-[13px] transition-all duration-300 ease-in-out mr-2"
        :class="[
          internalChecked
            ? 'border-(--accent-color) bg-[rgba(121,217,255,0.3)] shadow-[0_0_10px_rgba(121,217,255,0.3)]'
            : 'border-white/30 bg-white/20',
        ]"
      >
        <span
          class="absolute top-1/2 -translate-y-1/2 w-5 h-5 rounded-full transition-all duration-300 ease-in-out"
          :class="[
            internalChecked
              ? 'left-6.5 bg-linear-to-br from-(--accent-color) to-[#64b5f6] shadow-[0_3px_8px_rgba(121,217,255,0.4),0_1px_3px_rgba(0,0,0,0.2)]'
              : 'left-1 bg-linear-to-br from-white to-[#f0f0f0] shadow-[0_2px_6px_rgba(0,0,0,0.3),0_1px_2px_rgba(0,0,0,0.1)]',
          ]"
        ></span>
      </span>
      <slot></slot>
    </label>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'

const props = defineProps({
  checked: {
    type: Boolean,
    default: false,
  },
})

const emit = defineEmits(['change'])

const id = ref(`toggle-${Math.random().toString(36).substring(2, 9)}`)
const internalChecked = ref(props.checked)

watch(
  () => props.checked,
  (newVal) => {
    internalChecked.value = newVal
  },
)

const handleChange = (e: Event) => {
  const target = e.target as HTMLInputElement
  internalChecked.value = target.checked
  emit('change', target.checked)
}
</script>

<style scoped>
/* 保留CSS变量引用 */
:deep(*) {
  --accent-color: var(--accent-color);
}
</style>
