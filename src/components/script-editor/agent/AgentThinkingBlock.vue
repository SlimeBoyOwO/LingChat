<script setup lang="ts">
  /**
   * 思考/规划弱化折叠块。
   *
   * 承载两类「思考」文本，以不显眼的折叠形态展示：
   * - 真正的思考链（thinking 模式开启时由 LLM 产出，reasoning 事件流式到达）；
   * - 工具调用前的模型叙述（"Let me look at…" 之类，内容本身在 content 里）。
   * 折叠态只有一行浅色 pill，展开才显示灰字正文。纯文本渲染（散文无需 markdown）。
   */
  import { ref } from "vue";
  import { useI18n } from "vue-i18n";

  defineProps<{
    text: string;
    /** 该条消息是否正在流式生成：思考中时 pill 显示转圈与「思考中」高亮态 */
    streaming?: boolean;
  }>();
  const { t } = useI18n();
  const expanded = ref(false);
</script>

<template>
  <div class="flex w-full max-w-[92%] flex-col items-start gap-1">
    <button
      class="inline-flex cursor-pointer items-center gap-1.5 rounded-full border px-2.5 py-0.5
        text-[0.72rem] transition-colors"
      :class="
        streaming
          ? 'border-brand/40 bg-brand/12 text-brand'
          : `hover:border-brand/30 border-white/10 bg-white/5 text-white/45 hover:bg-white/10
            hover:text-white/70`
      "
      @click="expanded = !expanded"
    >
      <!-- 思考中：转圈指示（CSS 圆环动画，不引入图标依赖） -->
      <span
        v-if="streaming"
        class="h-2.5 w-2.5 shrink-0 animate-spin rounded-full border-[1.5px] border-current
          border-t-transparent"
      ></span>
      <span class="text-[0.6rem] leading-none">{{ expanded ? "▼" : "▶" }}</span>
      <span>{{
        streaming ? t("scriptEditor.agentThinking.thinking") : t("scriptEditor.agentThinking.label")
      }}</span>
    </button>
    <div
      v-if="expanded"
      class="max-h-64 w-full overflow-y-auto rounded-lg border border-white/10 bg-black/20 px-3 py-2
        text-[0.78rem] leading-relaxed whitespace-pre-wrap text-white/55"
    >
      {{ text }}
    </div>
  </div>
</template>
