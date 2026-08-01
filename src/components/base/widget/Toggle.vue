<template>
  <div class="flex items-center">
    <input
      type="checkbox"
      :id="id"
      :checked="internalChecked"
      :disabled="disabled"
      @change="handleChange"
      class="hidden"
    />
    <label
      :for="id"
      class="relative text-white text-3.5 select-none inline-flex items-center w-full"
      :class="disabled ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'"
      style="text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3)"
    >
      <span
        class="relative inline-block w-12.5 h-6.5 shrink-0 rounded-[13px] transition-all duration-300 ease-in-out mr-2"
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
      <span class="min-w-0 flex-1"><slot></slot></span>
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
  /// 禁用开关。禁用时原生 input 也会被 disabled，避免点穿；
  /// 视觉上变灰且鼠标变 not-allowed。默认 false，既有调用方不受影响。
  disabled: {
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
