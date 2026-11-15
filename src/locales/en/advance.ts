// English language pack, maintained from zh-CN/advance.ts
export default {
  tabs: {
    menu: "Main Menu",
    llm: "LLM Management",
    tools: "Tool Settings",
    other: "Other Advanced Settings",
    tts: "TTS Settings",
    asr: "Speech Recognition",
    cast: "Cast Settings",
  },
  menu: {
    llmTitle: "LLM Management",
    llmDesc:
      "Configure and manage AI LLM providers — add, edit, and test model connection parameters",
    llmButton: "Open LLM Management",
    toolsTitle: "Tool Settings",
    toolsDesc: "Configure the tools AI can use, like web search (looking things up online)",
    toolsButton: "Open Tool Settings",
    ttsTitle: "TTS Settings",
    ttsDesc:
      "Download and manage DeBERTa and character voice models to use the local TTS engine offline, or use cloud voice cloning (CosyVoice) to create your own voices",
    ttsButton: "Enter TTS Settings",
    asrTitle: "Speech Recognition",
    asrDesc: "Configure Alibaba Cloud ASR, send mode, and microphone auto-listen",
    asrButton: "Open Speech Recognition",
    otherTitle: "Other Advanced Settings",
    otherDesc:
      "Tweak system environment settings, prompt templates, chat parameters, and more (takes effect after restart)",
    otherButton: "Open Other Advanced Settings",
    languageTitle: "Interface Language",
    languageDesc: "Switch the app's display language — applies instantly and saves automatically",
    guideTitle: "Built-in TTS Guide",
    guideDesc:
      "View the built-in TTS tutorial (downloading models, importing voices, switching devices, etc.)",
    guideButton: "Open guide",
    castTitle: "Cast Settings",
    castDesc:
      "Stream the game scene to LAN devices — configure the port, resolution, and picture tuning",
    castButton: "Open Cast Settings",
    memoryTitle: "Permanent Memory Debug",
    memoryDesc:
      "Inspect a role's memory bank and the context actually sent to the LLM, for diagnosing memory compaction and injection",
    memoryButton: "Open Memory Debug",
    affectionTitle: "Affection System",
    affectionDesc:
      "Master switch for the affection system (applies immediately); heartbeat and wave animation toggles are under Other Advanced Settings",
    affectionMasterToggle: "Enable affection system",
    affectionToggleConfirm:
      "Toggle the affection system? When disabled, affection evaluation stops and the affection panel is hidden immediately.",
    affectionToggleFailed: "Failed to toggle the affection system, please try again later",
  },
  search: {
    placeholder:
      "Search settings, sub-options, or config keys (e.g. TTS, WEATHER_CITY, CosyVoice, hotkey…)",
    clear: "Clear search",
    resultCount: "{count} matching settings",
    resultCountCapped: "Showing top {shown} of {total} results",
    loadingDynamic: "Indexing all advanced config keys…",
    emptyTitle: "No matching settings found",
    emptyHint:
      "Try another keyword, module name, or raw key (e.g. weather, timeout, voice, hotword, cast)",
    shortcutHint: "Ctrl+K Focus · ↑↓ Select · Enter Jump · Esc Clear",
    jumpLabel: "Open",
    restartTag: "Restart required",
    sections: {
      menu: "Main Menu",
      llm: "LLM Providers",
      tts: "TTS Settings",
      asr: "Speech Recognition",
      tools: "Tool Settings",
      cast: "Screen Cast",
      memory: "Memory Debug",
      other: "Other Advanced",
    },
    descs: {
      menuLanguage: "Switch the UI display language; applies immediately and auto-saves",
      menuAffection: "Master switch for the affection system; applies immediately on save",
      menuGuide: "View the built-in TTS setup guide and tutorial",
      llmList: "Inspect, add, edit, delete, and test configured LLM endpoints",
      llmAdd: "Create a new OpenAI-compatible, LM Studio, or Codex provider",
      llmRoleChat: "Choose the primary model used for main conversations",
      llmRoleTranslate: "Choose the model used for bilingual dialogue translation",
      llmRoleGod: "Choose the lightweight decision model for background checks & tool picking",
      llmRoleVision:
        "Choose the multimodal model for screen perception and image/video understanding",
      ttsSwitch: "Enable or disable the offline local TTS engine",
      ttsInstalled: "Manage installed local character voice models",
      toolsWebSearch:
        "Configure Keenable / Kimi / BoCha / DeepSeek / Tavily / Codex and proxy settings",
      textAutoAdvance: "Adjust the pause interval when auto-advance dialogue is enabled",
      textTtsCache: "Check and clean orphaned local TTS audio cache files",
      charList: "Switch characters, edit character configs, or create a new character",
      achievement: "View unlocked and locked achievements",
    },
  },
};
