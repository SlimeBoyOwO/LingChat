<template>
  <div class="flex items-center gap-2" :class="{ 'opacity-40': disabled }">
    <span class="w-[5.5rem] shrink-0 text-[11px] text-white/50">{{ label }}</span>
    <input
      v-model="local"
      type="color"
      :disabled="disabled"
      class="h-6 w-9 shrink-0 cursor-pointer rounded border-0 bg-transparent p-0"
    />
    <span class="min-w-0 flex-1 truncate text-[11px] text-white/30">{{ local }}</span>
  </div>
</template>

<script setup lang="ts">
  import { ref, watch } from "vue";

  const props = defineProps<{
    modelValue: string;
    label: string;
    disabled?: boolean;
  }>();

  const emit = defineEmits<{ "update:modelValue": [value: string] }>();

  // `<input type=color>` 只吃 #rrggbb：预设里若出现别的写法就先归一，否则控件会
  // 静默退回黑色，用户看到的是「颜色变了但选择器没变」。
  const local = ref(normalize(props.modelValue));

  watch(
    () => props.modelValue,
    (v) => {
      local.value = normalize(v);
    }
  );

  watch(local, (v) => emit("update:modelValue", v));

  function normalize(hex: string): string {
    const v = (hex ?? "").trim();
    const six = /^#?([0-9a-f]{6})$/i.exec(v);
    if (six) return `#${six[1].toLowerCase()}`;
    const three = /^#?([0-9a-f])([0-9a-f])([0-9a-f])$/i.exec(v);
    if (three)
      return `#${three
        .slice(1)
        .map((c) => c.toLowerCase().repeat(2))
        .join("")}`;
    return v || "#000000";
  }
</script>
