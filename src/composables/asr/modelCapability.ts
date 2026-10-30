import { computed } from "vue";

import { useAsrStore } from "@/stores/modules/settings/asr";

/** 当前实际生效的识别模型的热词能力；null = 模型清单未加载。
 *  流式生效时取流式模型，否则取批量模型（与 gates.ts 的 isStreamEnabled 同源）。 */
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
      // 即时热词或 prompt 偏置任一可用即可
      supportsHotwords: m.supports_inline_vocabulary || m.supports_prompt_bias,
    };
  });
}
