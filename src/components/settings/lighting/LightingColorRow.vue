<template>
  <div class="flex items-center gap-2" :class="{ 'opacity-40': disabled }">
    <span class="w-[5.5rem] shrink-0 text-[11px] text-white/50">{{ label }}</span>
    <input
      v-model="local"
      type="color"
      :disabled="disabled"
      class="h-6 w-9 shrink-0 cursor-pointer rounded border-0 bg-transparent p-0"
    />
    <!-- 色号点进去就能直接敲：从 AIGC 工具抄来的 #RRGGBB 比拖色轮准得多 -->
    <input
      v-model="text"
      type="text"
      spellcheck="false"
      class="min-w-0 flex-1 rounded border border-transparent bg-transparent px-1 py-0.5 text-[11px]
        text-white/30 uppercase transition-colors hover:border-white/15 hover:text-white/60
        focus:border-amber-400/60 focus:bg-black/30 focus:text-white focus:outline-none"
      :disabled="disabled"
      @focus="editing = true"
      @blur="commit"
      @keydown.enter.prevent="commit"
      @keydown.esc.prevent="cancel"
    />
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
  // 色号框「显示 + 手敲」两用：敲到一半的字符串先留在 text 里，认得出
  // #rgb/#rrggbb 才写回 local（进而写进灯光参数），敲坏就弹回原色号。
  const editing = ref(false);
  const text = ref(local.value);

  watch(
    () => props.modelValue,
    (v) => {
      local.value = normalize(v);
      if (!editing.value) text.value = local.value;
    }
  );

  // 从色轮改的颜色要回填色号；手敲到一半不能被打断。
  watch(local, (v) => {
    emit("update:modelValue", v);
    if (!editing.value) text.value = v;
  });

  function commit(): void {
    editing.value = false;
    const next = tryNormalize(text.value);
    if (next) local.value = next;
    text.value = local.value;
  }

  function cancel(): void {
    editing.value = false;
    text.value = local.value;
  }

  function normalize(hex: string): string {
    const trimmed = (hex ?? "").trim();
    return tryNormalize(trimmed) ?? (trimmed || "#000000");
  }

  /** 认 #rgb / #rrggbb（# 可省），认不出来返回 null */
  function tryNormalize(hex: string): string | null {
    const v = (hex ?? "").trim();
    const six = /^#?([0-9a-f]{6})$/i.exec(v);
    if (six) return `#${six[1].toLowerCase()}`;
    const three = /^#?([0-9a-f])([0-9a-f])([0-9a-f])$/i.exec(v);
    if (three)
      return `#${three
        .slice(1)
        .map((c) => c.toLowerCase().repeat(2))
        .join("")}`;
    return null;
  }
</script>
