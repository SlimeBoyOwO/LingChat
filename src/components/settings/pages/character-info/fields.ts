import type { ComposerTranslation } from "vue-i18n";

export type FieldType = "text" | "number" | "textarea" | "examples" | "select" | "switch";

export interface FieldOption {
  label: string;
  value: string;
  visibleIf?: (settings: any) => boolean;
}

export interface FieldSchema {
  key: string;
  label: string;
  type: FieldType;
  rows?: number;
  step?: string;
  min?: number;
  max?: number;
  placeholder?: string;
  /** 字段下方的小字说明，渲染在控件下方 */
  hint?: string;
  /** 允许留空：空值写入 undefined，而非被 Number() 强转成 0 */
  nullable?: boolean;
  options?: FieldOption[];
  // Dynamic options computed from refs/state; overrides options when set.
  dynamicOptions?: () => { label: string; value: string }[];
  visibleIf?: (settings: any) => boolean;
  isVoiceModel?: boolean;
  // When set, the field reads/writes into localSettings.value[parent][key].
  parent?: string;
}

/** 构建表单 schema 需要的运行期上下文：文案、形象模式选项、可用音色。 */
export interface SchemaContext {
  t: ComposerTranslation;
  avatarModeOptions: FieldOption[];
  cloudVoices: any[];
  installedVoices: any[];
}

/** 角色信息页各 tab 的字段定义。纯数据，不含组件状态。 */
export function buildSchemas(ctx: SchemaContext): Record<string, FieldSchema[]> {
  const { t, avatarModeOptions, cloudVoices, installedVoices } = ctx;
  return {
    basic: [
      { key: "ai_name", label: t("settings.characterInfo.fields.aiName"), type: "text" },
      { key: "ai_subtitle", label: t("settings.characterInfo.fields.aiSubtitle"), type: "text" },
      {
        key: "user_name",
        label: t("settings.characterInfo.fields.userName"),
        type: "text",
      },
      {
        key: "user_subtitle",
        label: t("settings.characterInfo.fields.userSubtitle"),
        type: "text",
      },
      {
        key: "initial_affection",
        label: t("settings.characterInfo.fields.initialAffection"),
        type: "number",
        step: "1",
        min: -100,
        max: 100,
        nullable: true,
        hint: t("settings.characterInfo.fields.initialAffectionHint"),
      },
      { key: "title", label: t("settings.characterInfo.fields.title"), type: "text" },
      { key: "info", label: t("settings.characterInfo.fields.info"), type: "textarea", rows: 4 },
    ],
    prompts: [
      {
        key: "system_prompt",
        label: t("settings.characterInfo.fields.systemPrompt"),
        type: "textarea",
        rows: 10,
      },
      {
        key: "system_prompt_example",
        label: t("settings.characterInfo.fields.systemPromptExample"),
        type: "examples",
        rows: 6,
      },
      {
        key: "system_prompt_example_old",
        label: t("settings.characterInfo.fields.systemPromptExampleOld"),
        type: "examples",
        rows: 4,
      },
    ],
    visuals: [
      {
        key: "avatar_mode",
        label: t("settings.characterInfo.fields.avatarMode"),
        type: "select",
        // 没有 Live2D 模型时整项隐藏（schemas 是 computed，导入模型后自动出现）
        visibleIf: (s) => Boolean(s.live2d),
        options: avatarModeOptions,
      },
      {
        key: "scale",
        label: t("settings.characterInfo.fields.scale"),
        type: "number",
        step: "0.01",
      },
      {
        key: "offset_x",
        label: t("settings.characterInfo.fields.offsetX"),
        type: "number",
        step: "0.1",
      },
      {
        key: "offset_y",
        label: t("settings.characterInfo.fields.offsetY"),
        type: "number",
        step: "0.1",
      },
      { key: "bubble_top", label: t("settings.characterInfo.fields.bubbleTop"), type: "number" },
      { key: "bubble_left", label: t("settings.characterInfo.fields.bubbleLeft"), type: "number" },
      {
        key: "thinking_message",
        label: t("settings.characterInfo.fields.thinkingMessage"),
        type: "text",
      },
    ],
    pet: [
      {
        key: "avatar_mode_p",
        label: t("settings.characterInfo.fields.avatarModeP"),
        type: "select",
        visibleIf: (s) => Boolean(s.live2d),
        options: avatarModeOptions,
      },
      {
        key: "pet_frameless",
        label: t("settings.characterInfo.fields.petFrameless"),
        type: "switch",
        hint: t("settings.characterInfo.fields.petFramelessHint"),
      },
      {
        key: "scale_p",
        label: t("settings.characterInfo.fields.scaleP"),
        type: "number",
        step: "0.01",
      },
      {
        key: "offset_x_p",
        label: t("settings.characterInfo.fields.offsetXP"),
        type: "number",
        step: "0.1",
      },
      {
        key: "offset_y_p",
        label: t("settings.characterInfo.fields.offsetYP"),
        type: "number",
        step: "0.1",
      },
    ],
    voice: [
      {
        key: "tts_type",
        label: t("settings.characterInfo.fields.ttsType"),
        type: "select",
        options: [
          { label: "sva", value: "sva-vits" },
          { label: "sbv2", value: "sbv2" },
          { label: "bv2", value: "sva-bv2" },
          { label: "sbv2api", value: "sbv2api" },
          { label: "gsv", value: "gsv" },
          { label: "aivis", value: "aivis" },
          { label: "opentts", value: "opentts" },
          { label: t("settings.characterInfo.fields.fishS2"), value: "fishs2" },
          { label: t("settings.characterInfo.fields.localSbv2Api"), value: "localsbv2api" },
          { label: "indextts2", value: "indextts2" },
          { label: t("settings.characterInfo.fields.voiceCloneTts"), value: "cosyvoice" },
        ],
      },

      {
        key: "voice_lang",
        label: t("settings.characterInfo.fields.voiceLang"),
        type: "select",
        options: [
          // 顺序:中、英、日、德、法、俄、韩、葡(用户指定);es/ar 仅 indextts2 可见
          { label: t("settings.characterInfo.voiceLangOptions.zh"), value: "zh" },
          {
            label: t("settings.characterInfo.voiceLangOptions.en"),
            value: "en",
            visibleIf: (s) =>
              ["gsv", "opentts", "sbv2", "sbv2api", "indextts2", "fishs2", "cosyvoice"].includes(
                s.tts_type,
              ),
          },
          { label: t("settings.characterInfo.voiceLangOptions.ja"), value: "ja" },
          {
            label: t("settings.characterInfo.voiceLangOptions.de"),
            value: "de",
            visibleIf: (s) => s.tts_type === "cosyvoice",
          },
          {
            label: t("settings.characterInfo.voiceLangOptions.fr"),
            value: "fr",
            visibleIf: (s) => s.tts_type === "cosyvoice",
          },
          {
            label: t("settings.characterInfo.voiceLangOptions.ru"),
            value: "ru",
            visibleIf: (s) => s.tts_type === "cosyvoice",
          },
          {
            label: t("settings.characterInfo.voiceLangOptions.ko"),
            value: "ko",
            visibleIf: (s) => ["gsv", "opentts", "cosyvoice"].includes(s.tts_type),
          },
          {
            label: t("settings.characterInfo.voiceLangOptions.pt"),
            value: "pt",
            visibleIf: (s) => s.tts_type === "cosyvoice",
          },
          {
            label: t("settings.characterInfo.voiceLangOptions.es"),
            value: "es",
            visibleIf: (s) => s.tts_type === "indextts2",
          },
          {
            label: t("settings.characterInfo.voiceLangOptions.ar"),
            value: "ar",
            visibleIf: (s) => s.tts_type === "indextts2",
          },
        ],
      },

      {
        // 中文方言：仅语音克隆 TTS + 中文时可选
        key: "voice_dialect",
        label: t("settings.characterInfo.fields.voiceDialect"),
        type: "select",
        options: [
          { label: t("settings.characterInfo.dialectOptions.mandarin"), value: "" },
          { label: t("settings.characterInfo.dialectOptions.cantonese"), value: "广东话" },
          { label: t("settings.characterInfo.dialectOptions.dongbei"), value: "东北话" },
          { label: t("settings.characterInfo.dialectOptions.gansu"), value: "甘肃话" },
          { label: t("settings.characterInfo.dialectOptions.guizhou"), value: "贵州话" },
          { label: t("settings.characterInfo.dialectOptions.henan"), value: "河南话" },
          { label: t("settings.characterInfo.dialectOptions.hubei"), value: "湖北话" },
          { label: t("settings.characterInfo.dialectOptions.jiangxi"), value: "江西话" },
          { label: t("settings.characterInfo.dialectOptions.fujian"), value: "闽南话" },
          { label: t("settings.characterInfo.dialectOptions.ningxia"), value: "宁夏话" },
          { label: t("settings.characterInfo.dialectOptions.shanxi"), value: "山西话" },
          { label: t("settings.characterInfo.dialectOptions.shaanxi"), value: "陕西话" },
          { label: t("settings.characterInfo.dialectOptions.shandong"), value: "山东话" },
          { label: t("settings.characterInfo.dialectOptions.shanghai"), value: "上海话" },
          { label: t("settings.characterInfo.dialectOptions.sichuan"), value: "四川话" },
          { label: t("settings.characterInfo.dialectOptions.tianjin"), value: "天津话" },
          { label: t("settings.characterInfo.dialectOptions.yunnan"), value: "云南话" },
        ],
        visibleIf: (s) => s.tts_type === "cosyvoice" && s.voice_lang === "zh",
      },

      {
        key: "sva_speaker_id",
        label: "sva_speaker_id",
        type: "text",
        isVoiceModel: true,
        visibleIf: (s) => s.tts_type === "sva-vits",
      },

      {
        key: "sbv2_name",
        label: "sbv2_name",
        type: "text",
        isVoiceModel: true,
        visibleIf: (s) => s.tts_type === "sbv2",
      },
      {
        key: "sbv2_speaker_id",
        label: "sbv2_speaker_id",
        type: "text",
        isVoiceModel: true,
        visibleIf: (s) => s.tts_type === "sbv2",
      },

      {
        key: "bv2_speaker_id",
        label: "bv2_speaker_id",
        type: "text",
        isVoiceModel: true,
        visibleIf: (s) => s.tts_type === "sva-bv2",
      },

      {
        key: "sbv2api_name",
        label: "sbv2api_name",
        type: "text",
        isVoiceModel: true,
        visibleIf: (s) => s.tts_type === "sbv2api",
      },
      {
        key: "sbv2api_speaker_id",
        label: "sbv2api_speaker_id",
        type: "text",
        isVoiceModel: true,
        visibleIf: (s) => s.tts_type === "sbv2api",
      },

      {
        key: "gsv_voice_text",
        label: "gsv_voice_text",
        type: "text",
        isVoiceModel: true,
        visibleIf: (s) => s.tts_type === "gsv",
      },
      {
        key: "gsv_voice_filename",
        label: "gsv_voice_filename",
        type: "text",
        isVoiceModel: true,
        visibleIf: (s) => s.tts_type === "gsv",
      },
      {
        key: "gsv_gpt_model_name",
        label: "gsv_gpt_model_name",
        type: "text",
        isVoiceModel: true,
        visibleIf: (s) => s.tts_type === "gsv",
      },
      {
        key: "gsv_sovits_model_name",
        label: "gsv_sovits_model_name",
        type: "text",
        isVoiceModel: true,
        visibleIf: (s) => s.tts_type === "gsv",
      },

      {
        key: "opentts_voice",
        label: t("settings.characterInfo.fields.openttsVoice"),
        type: "text",
        isVoiceModel: true,
        placeholder: t("settings.characterInfo.placeholders.openttsVoice"),
        visibleIf: (s) => s.tts_type === "opentts",
      },

      {
        key: "aivis_model_uuid",
        label: "aivis_model_uuid",
        type: "text",
        isVoiceModel: true,
        visibleIf: (s) => s.tts_type === "aivis",
      },

      {
        key: "fish_s2_voice",
        label: t("settings.characterInfo.fields.fishS2Voice"),
        type: "text",
        isVoiceModel: true,
        placeholder: t("settings.characterInfo.placeholders.fishS2Voice"),
        visibleIf: (s) => s.tts_type === "fishs2",
      },

      // --- 云端语音克隆 (cosyvoice) ---
      {
        key: "cosyvoice_voice_id",
        parent: "voice_models",
        label: t("settings.characterInfo.fields.cosyVoiceVoice"),
        type: "select",
        dynamicOptions: () =>
          cloudVoices.length === 0
            ? [{ label: t("settings.characterInfo.fields.noCloudVoice"), value: "" }]
            : cloudVoices.map((voice) => ({
                label: voice.name ? `${voice.name} (${voice.voice_id})` : voice.voice_id,
                value: voice.voice_id,
              })),
        visibleIf: (s) => s.tts_type === "cosyvoice",
      },

      // --- Local SBV2 (localsbv2api) ---
      {
        key: "sbv2_local_voice_id",
        parent: "voice_models",
        label: t("settings.characterInfo.fields.localVoiceId"),
        type: "select",
        dynamicOptions: () =>
          installedVoices.length === 0
            ? [{ label: t("settings.characterInfo.fields.noLocalModel"), value: "" }]
            : installedVoices.map((voice) => ({
                label: voice.display_name
                  ? `${voice.display_name} (${voice.voice_id})`
                  : voice.voice_id,
                value: voice.voice_id,
              })),
        visibleIf: (s) => s.tts_type === "localsbv2api",
      },
      {
        key: "sbv2_local_speaker_id",
        parent: "voice_models",
        label: t("settings.characterInfo.fields.speakerId"),
        type: "number",
        step: "1",
        visibleIf: (s) => s.tts_type === "localsbv2api",
      },
      {
        key: "sbv2_local_style_id",
        parent: "voice_models",
        label: t("settings.characterInfo.fields.styleId"),
        type: "number",
        step: "1",
        visibleIf: (s) => s.tts_type === "localsbv2api",
      },
      {
        key: "sbv2_local_length_scale",
        parent: "voice_models",
        label: t("settings.characterInfo.fields.lengthScale"),
        type: "number",
        step: "0.05",
        visibleIf: (s) => s.tts_type === "localsbv2api",
      },
      {
        key: "sbv2_local_sdp_ratio",
        parent: "voice_models",
        label: t("settings.characterInfo.fields.sdpRatio"),
        type: "number",
        step: "0.05",
        visibleIf: (s) => s.tts_type === "localsbv2api",
      },
      {
        key: "sbv2_local_cloud_fallback_model",
        parent: "voice_models",
        label: t("settings.characterInfo.fields.cloudFallbackModel"),
        type: "text",
        placeholder: t("settings.characterInfo.fields.cloudFallbackPlaceholder"),
        visibleIf: (s) => s.tts_type === "localsbv2api",
      },
      {
        key: "sbv2_local_cloud_fallback_speaker_id",
        parent: "voice_models",
        label: t("settings.characterInfo.fields.cloudFallbackSpeakerId"),
        type: "text",
        placeholder: t("settings.characterInfo.fields.cloudFallbackPlaceholder"),
        visibleIf: (s) => s.tts_type === "localsbv2api",
      },
      {
        key: "opentts_voice",
        label: t("settings.characterInfo.fields.openttsVoiceLabel"),
        type: "text",
        isVoiceModel: true,
        placeholder: t("settings.characterInfo.fields.openttsVoicePlaceholder"),
        visibleIf: (s) => s.tts_type === "opentts",
      },
    ],
  };
}
