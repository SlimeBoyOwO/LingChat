<script setup lang="ts">
/**
 * 思考/规划弱化折叠块。
 *
 * 承载两类「思考」文本，以不显眼的折叠形态展示：
 * - 真正的思考链（thinking 模式开启时由 LLM 产出，reasoning 事件流式到达）；
 * - 工具调用前的模型叙述（"Let me look at…" 之类，内容本身在 content 里）。
 * 折叠态只有一行浅色 pill，展开才显示灰字正文。纯文本渲染（散文无需 markdown）。
 */
import { ref } from 'vue'

defineProps<{ text: string }>()
const expanded = ref(false)
</script>

<template>
  <div class="flex w-full max-w-[92%] flex-col items-start gap-1">
    <button
      class="inline-flex cursor-pointer items-center gap-1.5 rounded-full border border-white/10 bg-white/5 px-2.5 py-0.5 text-[0.72rem] text-white/45 transition-colors hover:border-brand/30 hover:bg-white/10 hover:text-white/70"
      @click="expanded = !expanded"
    >
      <span class="text-[0.6rem] leading-none">{{ expanded ? '▼' : '▶' }}</span>
      <span>思考/规划…</span>
    </button>
    <div
      v-if="expanded"
      class="max-h-64 w-full overflow-y-auto whitespace-pre-wrap rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-[0.78rem] leading-relaxed text-white/55"
    >
      {{ text }}
    </div>
  </div>
</template>
