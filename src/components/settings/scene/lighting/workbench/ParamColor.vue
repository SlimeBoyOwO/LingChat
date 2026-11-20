<script setup lang="ts">
/** 颜色参数行：色块 + hex + 可选百分比（强度占比，纯展示）。 */
import { computed, ref, watch } from "vue";

const props = defineProps<{
  modelValue: string;
  label: string;
  percent?: number;
  hint?: string;
}>();

const emit = defineEmits<{ (e: "update:modelValue", v: string): void }>();

const text = ref(normalize(props.modelValue));

function normalize(v: string): string {
  const s = (v || "").trim().replace(/^#/, "");
  return s ? `#${s.toUpperCase()}` : "#FFFFFF";
}

watch(
  () => props.modelValue,
  (v) => {
    if (normalize(v) !== text.value.toUpperCase()) text.value = normalize(v);
  },
);

const label2 = computed(() => text.value.replace(/^#/, ""));

function onPick(e: Event) {
  emit("update:modelValue", (e.target as HTMLInputElement).value);
}

function onHexInput() {
  const raw = text.value.trim().replace(/^#/, "");
  if (/^[0-9a-fA-F]{6}$/.test(raw)) emit("update:modelValue", `#${raw}`);
}
</script>

<template>
  <div class="wb-param" style="padding-top: 6px; padding-bottom: 6px">
    <div class="wb-param-top">
      <span class="wb-param-label" :title="hint || label">{{ label }}</span>
      <span class="wb-param-value">
        <label class="wb-swatch" :style="{ background: text }">
          <input type="color" :value="text" @input="onPick" />
        </label>
        <input
          v-model="text"
          class="wb-num"
          style="width: 62px; text-align: left"
          spellcheck="false"
          @input="onHexInput"
        />
        <span v-if="percent !== undefined" class="wb-hex" style="margin-left: 2px"
          >{{ Math.round(percent) }}%</span
        >
      </span>
    </div>
    <span class="sr-only">{{ label2 }}</span>
  </div>
</template>
