import { computed } from "vue";

import { useAsrStore } from "@/stores/modules/settings/asr";

/**
 * 当前**实际生效**的识别模型的热词能力；`null` = 清单未加载（不提示：宁可漏报，也不误报）。
 *
 * 流式开关生效时用流式模型，否则用批量模型（配置为空、或配的模型不支持流式时，
 * 后端回退成该地域的批量默认模型，见 `providers/qwen.rs` 的 effective_batch_model）。
 * 判定与 `composables/asr/gates.ts` 的 `isStreamEnabled` 同源——热词能力要按真正在跑的那个
 * 模型取。角色热词 tab 与全局热词弹窗共用。
 */
export function useAsrModelCapability() {
  const asrStore = useAsrStore();

  const activeAsrModel = computed(() => {
    const s = asrStore.settings;
    const models = asrStore.models;
    const selected =
      models.find((m) => m.id === (s.provider_configs[s.active_provider]?.model ?? "").trim()) ??
      models.find((m) => m.is_default);
    if (s.stream_enabled && selected?.supports_streaming) return selected;
    if (!selected || selected.supports_streaming) return models.find((m) => m.is_default);
    return selected;
  });

  return computed(() => {
    const m = activeAsrModel.value;
    if (!m) return null;
    return {
      modelId: m.id,
      // 即时热词与 prompt 偏置：任一能把热词带上就不算「配了不生效」
      supportsHotwords: m.supports_inline_vocabulary || m.supports_prompt_bias,
    };
  });
}
