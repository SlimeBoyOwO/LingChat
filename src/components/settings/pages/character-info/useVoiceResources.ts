import { computed, ref, type Ref } from "vue";
import * as TtsLocal from "@/api/services/tts/tts-local";
import * as TtsCosyvoice from "@/api/services/tts/tts-cosyvoice";

/** 语音模型字段的旧位置清单，迁移时统一搬进 voice_models。 */
const voiceModelKeys = [
  "sva_speaker_id",
  "sbv2_name",
  "sbv2_speaker_id",
  "bv2_speaker_id",
  "sbv2api_name",
  "sbv2api_speaker_id",
  "gsv_voice_text",
  "gsv_voice_filename",
  "gsv_gpt_model_name",
  "gsv_sovits_model_name",
  "aivis_model_uuid",
  "opentts_voice",
  "fish_s2_voice",
  "cosyvoice_voice_id",
] as const;

const GSV_EMO_CATEGORIES = ["吃惊", "开心", "恐惧", "难过", "生气", "中立"] as const;

/** 角色语音资源：音色列表、语音模型字段迁移与 GSV 六情绪参考语音的草稿读写。 */
export function useVoiceResources(localSettings: Ref<any>) {
  const installedVoices = ref<TtsLocal.VoiceRecord[]>([]);
  const cloudVoices = ref<TtsCosyvoice.CosyVoiceView[]>([]);

  async function refreshLocalVoices(): Promise<void> {
    try {
      const snapshot = await TtsLocal.listInstalled();
      installedVoices.value = snapshot.voices;
    } catch (error) {
      console.warn("refreshLocalVoices failed", error);
      installedVoices.value = [];
    }
  }

  async function refreshCloudVoices(): Promise<void> {
    try {
      cloudVoices.value = await TtsCosyvoice.listVoices();
    } catch (error) {
      console.warn("refreshCloudVoices failed", error);
      cloudVoices.value = [];
    }
  }

  const ensureVoiceModels = () => {
    if (
      !localSettings.value.voice_models ||
      typeof localSettings.value.voice_models !== "object" ||
      Array.isArray(localSettings.value.voice_models)
    ) {
      localSettings.value.voice_models = {};
    }
    return localSettings.value.voice_models as Record<string, unknown>;
  };

  const migrateLegacyVoiceModelFields = () => {
    const voiceModels = ensureVoiceModels();
    for (const key of voiceModelKeys) {
      const legacyValue = localSettings.value[key];
      if ((voiceModels[key] === undefined || voiceModels[key] === null) && legacyValue != null) {
        voiceModels[key] = legacyValue;
      }
      delete localSettings.value[key];
    }
  };

  const gsvEmoEnabled = computed({
    get: () => Boolean(localSettings.value.voice_models?.gsv_emo_enabled),
    set: (val: boolean) => {
      ensureVoiceModels().gsv_emo_enabled = val;
    },
  });

  const toggleGsvEmo = () => {
    gsvEmoEnabled.value = !gsvEmoEnabled.value;
  };

  /** 分类参考值的读写入口，与 voice_models 下的平级字段同样只落草稿。 */
  const gsvEmoField = (cat: string, key: "gsv_emo_texts" | "gsv_emo_voice_files") =>
    computed({
      get: () => (ensureVoiceModels()[key] as Record<string, string> | undefined)?.[cat] ?? "",
      set: (val: string) => {
        const voiceModels = ensureVoiceModels();
        const map = (voiceModels[key] as Record<string, string> | undefined) ?? {};
        map[cat] = val;
        voiceModels[key] = map;
      },
    });

  // 合成采样参数：空串删除键（留空=后端默认），非空转数字
  const gsvSamplingField = (key: "gsv_temperature" | "gsv_top_p" | "gsv_speed_factor") =>
    computed({
      get: () => {
        const v = ensureVoiceModels()[key];
        return v === undefined || v === null ? "" : String(v);
      },
      set: (val: string | number) => {
        const trimmed = String(val).trim();
        const voiceModels = ensureVoiceModels();
        if (trimmed === "") {
          delete voiceModels[key];
          return;
        }
        const n = Number(trimmed);
        if (Number.isFinite(n)) voiceModels[key] = n;
      },
    });

  const gsvTemperature = gsvSamplingField("gsv_temperature");
  const gsvTopP = gsvSamplingField("gsv_top_p");
  const gsvSpeedFactor = gsvSamplingField("gsv_speed_factor");

  return {
    installedVoices,
    cloudVoices,
    refreshLocalVoices,
    refreshCloudVoices,
    ensureVoiceModels,
    migrateLegacyVoiceModelFields,
    GSV_EMO_CATEGORIES,
    gsvEmoEnabled,
    toggleGsvEmo,
    gsvEmoField,
    gsvTemperature,
    gsvTopP,
    gsvSpeedFactor,
  };
}
