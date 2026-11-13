<template>
  <input
    :type="type"
    :value="model"
    class="focus:border-brand focus:ring-brand/20 mt-2 w-full rounded-lg border border-white/10 bg-white/10 px-3 py-2.5 text-sm text-white transition-colors duration-200 focus:ring-2 focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
    v-bind="$attrs"
    @input="onInput"
  />
</template>

<script setup lang="ts">
defineOptions({ inheritAttrs: false });

const props = withDefaults(defineProps<{ type?: "text" | "password" | "number" }>(), {
  type: "text",
});
const model = defineModel<string | number>({ required: true });

// 与原生 v-model.number 一致：解析失败时保留原始文本，清空时写回空串。
function onInput(event: Event) {
  const input = event.target as HTMLInputElement;
  if (props.type !== "number") {
    model.value = input.value;
    return;
  }
  const parsed = input.valueAsNumber;
  model.value = Number.isFinite(parsed) ? parsed : input.value;
}
</script>
