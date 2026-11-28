import { computed, ref, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { getEnvConfigSettings, type ConfigItem } from "@/api/services/config";
import { toolDescription, toolDisplayName } from "@/api/services/tool-settings";
import { useLlmProvidersStore } from "@/stores/modules/llm-providers";
import { useToolSettingsStore } from "@/stores/modules/settings/tools";
import { isMacOS, isMobile } from "@/utils/platform";
import type { AdvanceJumpTarget, AdvanceTab } from "./advanceJump";

export type SettingsSearchTarget = AdvanceJumpTarget;

export interface SettingsSearchItem {
  id: string;
  title: string;
  description: string;
  breadcrumbs: string[];
  rawKey?: string;
  restartRequired?: boolean;
  aliases: string[];
  target: AdvanceJumpTarget;
}

export interface SettingsSearchMatch {
  item: SettingsSearchItem;
  score: number;
}

export interface HighlightSegment {
  text: string;
  matched: boolean;
}

interface EnvConfigSubcategory {
  settings?: ConfigItem[];
}

interface EnvConfigCategory {
  subcategories?: Record<string, EnvConfigSubcategory>;
}

type EnvConfigTree = Record<string, EnvConfigCategory>;

const cachedEnvConfig = ref<EnvConfigTree | null>(null);
const loadingEnvConfig = ref(false);
let envConfigInFlight: Promise<void> | null = null;

const MAX_RENDERED_RESULTS = 45;
const SENSITIVE_KEY_RE = /(key|token|secret|password)/i;

const ENV_KEY_ALIASES: Record<string, string[]> = {
  "llm.output_sec_lang": ["双语", "第二语言", "外语", "日语", "英语", "bilingual"],
  "llm.consumers": ["并发", "流式", "consumer", "stream"],
  "llm.timeout_secs": ["超时", "等待", "idle", "timeout", "卡住"],
  "llm.no_emotion_limit_prompt": ["情绪", "标签限制", "token"],
  "translate.enable": ["翻译", "译文", "双语", "translate"],
  "features.enable_time_sense": ["时间感知", "早中晚", "报时"],
  "features.enable_emotion_classifier": ["情绪分类", "onnx", "表情识别"],
  "features.use_persistent_memory": ["永久记忆", "长期记忆", "记忆压缩", "summary"],
  "features.memory_update_interval": ["记忆触发", "压缩阈值", "摘要间隔", "250"],
  "features.memory_recent_window": ["最近窗口", "保留对话", "上下文窗口"],
  "tts.simple_vits_api_url": ["vits", "simple-vits", "url", "端口", "地址"],
  "tts.bv2_api_url": ["bert-vits2", "bv2", "url", "地址"],
  "tts.gsv_api_url": ["gpt-sovits", "gsv", "url", "端口", "地址"],
  "tts.sbv2_api_url": ["style-bert-vits2", "sbv2", "本地语音", "url"],
  "tts.sbv2api_api_url": ["sbv2api", "url", "地址"],
  "tts.aivis_api_url": ["aivis", "云端语音", "url"],
  "tts.aivis_api_key": ["aivis", "api key", "密钥"],
  "tts.indextts_api_url": ["indextts", "index", "url"],
  "tts.fish_s2_api_url": ["fish", "s2", "url"],
  "tts.fish_s2_voice": ["fish", "paimeng", "音色"],
  "tts.opentts_api_url": ["opentts", "siliconflow", "硅基流动", "url"],
  "tts.opentts_api_key": ["opentts", "siliconflow", "api key", "密钥"],
  "tts.opentts_model": ["opentts", "模型", "model"],
  "tts.opentts_voice": ["opentts", "音色", "voice"],
  "tts.audio_format": ["wav", "mp3", "flac", "ogg", "音频格式"],
  "workshop.github_token": ["github", "token", "创意工坊", "点赞", "排序", "pat"],
  "log.enable": ["文件日志", "落盘", "data/log", "log"],
  "log.retention_days": ["保留天数", "日志清理", "过期"],
  "log.llm_request_body": ["请求体", "request body", "json", "抓包", "调试"],
  "log.genai_debug": ["genai", "sdk", "debug", "即时生效"],
  ENABLE_PROACTIVE_SYSTEM: ["主动对话", "主动聊天", "proactive", "搭话"],
  MAX_PROACTIVE_TIMES: ["主动次数", "连续搭话", "上限"],
  PROACTIVE_EXPECTED_INTERVAL_MINUTES: ["主动间隔", "分钟", "频率", "兴趣增长"],
  ENABLE_VISUAL_PRECEPTION: ["看屏幕", "偷看", "桌面", "视觉感知", "截图"],
  SCREEN_WEIGHT: ["看屏幕权重", "视觉权重", "桌宠偷看"],
  ENABLE_TOPIC_CREATER: ["随机话题", "找话题", "topic"],
  TOPIC_WEIGHT: ["话题权重", "topic weight"],
  ENABLE_TODO_PRECEPTION: ["待办感知", "todo提醒", "待办提醒"],
  TODO_WEIGHT: ["待办权重", "todo weight"],
  ENABLE_SCHEDULE_REMINDER: ["日程提醒", "闹钟", "弹窗提醒"],
  ENABLE_IMPORTANT_DAY_REMINDER: ["重要日子", "纪念日", "节日", "生日"],
  WEATHER_CITY: ["天气", "城市", "下雨", "weather", "city"],
};

function mergeRanges(ranges: Array<[number, number]>): Array<[number, number]> {
  if (ranges.length === 0) return [];
  const sorted = [...ranges].sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const merged: Array<[number, number]> = [sorted[0]];
  for (let i = 1; i < sorted.length; i += 1) {
    const prev = merged[merged.length - 1];
    const curr = sorted[i];
    if (curr[0] <= prev[1]) prev[1] = Math.max(prev[1], curr[1]);
    else merged.push(curr);
  }
  return merged;
}

export function splitHighlightSegments(text: string, query: string): HighlightSegment[] {
  if (!text) return [];
  const tokens = query.trim().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return [{ text, matched: false }];

  const lowerText = text.toLowerCase();
  const ranges: Array<[number, number]> = [];
  for (const token of tokens) {
    const lowerToken = token.toLowerCase();
    let offset = 0;
    while (offset < lowerText.length) {
      const idx = lowerText.indexOf(lowerToken, offset);
      if (idx === -1) break;
      ranges.push([idx, idx + lowerToken.length]);
      offset = idx + Math.max(1, lowerToken.length);
    }
  }

  const merged = mergeRanges(ranges);
  if (merged.length === 0) return [{ text, matched: false }];

  const segments: HighlightSegment[] = [];
  let cursor = 0;
  for (const [start, end] of merged) {
    if (start > cursor) segments.push({ text: text.slice(cursor, start), matched: false });
    segments.push({ text: text.slice(start, end), matched: true });
    cursor = end;
  }
  if (cursor < text.length) segments.push({ text: text.slice(cursor), matched: false });
  return segments;
}

function scoreToken(
  token: string,
  titleLower: string,
  rawKeyLower: string,
  aliasesLower: string[],
  breadcrumbLower: string,
  descLower: string,
): number {
  let best = 0;

  if (rawKeyLower === token || titleLower === token) {
    best = Math.max(best, 145);
  } else {
    if (rawKeyLower.startsWith(token)) best = Math.max(best, 105);
    if (titleLower.startsWith(token)) best = Math.max(best, 95);
    const titleIdx = titleLower.indexOf(token);
    if (titleIdx >= 0) best = Math.max(best, Math.max(52, 74 - Math.min(titleIdx, 20)));
    if (rawKeyLower.includes(token)) best = Math.max(best, 58);
  }

  for (const alias of aliasesLower) {
    if (alias === token) best = Math.max(best, 56);
    else if (alias.startsWith(token)) best = Math.max(best, 44);
    else if (alias.includes(token)) best = Math.max(best, 32);
  }

  if (breadcrumbLower.includes(token)) best = Math.max(best, 22);
  if (descLower.includes(token)) best = Math.max(best, 15);
  return best;
}

function formatEnvValuePreview(key: string, value: unknown): string {
  if (SENSITIVE_KEY_RE.test(key)) return "";
  if (typeof value === "boolean") return value ? "ON" : "OFF";
  if (typeof value === "number") return String(value);
  if (typeof value === "string") {
    const trimmed = value.trim();
    return trimmed ? (trimmed.length > 36 ? `${trimmed.slice(0, 36)}…` : trimmed) : "";
  }
  return "";
}

function parseAliases(aliases: string | string[]): string[] {
  const list = typeof aliases === "string" ? aliases.split(",") : aliases;
  return list.map((s) => s.trim()).filter(Boolean);
}

interface AddItemOptions extends Omit<AdvanceJumpTarget, "advanceTab"> {
  crumbs?: string[];
  rawKey?: string;
  restartRequired?: boolean;
}

type AnchorRow = readonly [
  anchor: string,
  title: string,
  description: string,
  aliases: string,
  opts?: Omit<AddItemOptions, "anchor">,
];

const MENU_ROWS: AnchorRow[] = [
  [
    "menu-language",
    "menu.languageTitle",
    "search.descs.menuLanguage",
    "language, locale, i18n, 语言, 简体中文, 繁体中文, english, 日本語",
  ],
  [
    "menu-affection",
    "menu.affectionTitle",
    "search.descs.menuAffection",
    "好感, affection, 总开关, 好感度开关",
  ],
  [
    "menu-guide",
    "menu.guideTitle",
    "search.descs.menuGuide",
    "教程, 指南, guide, tutorial, tts教程",
  ],
];

const LLM_ROWS: AnchorRow[] = [
  [
    "llm-list",
    "list.title",
    "advance.search.descs.llmList",
    "llm, api, openai, deepseek, codex, lm studio, 大模型, 模型管理, api key, base url, 温度, temperature",
  ],
  [
    "llm-add",
    "list.addModel",
    "advance.search.descs.llmAdd",
    "add model, 新增模型, 添加api, 预设, preset, 接入模型",
  ],
  [
    "llm-role-chat",
    "role.chatModel",
    "advance.search.descs.llmRoleChat",
    "chat model, 对话模型, 主模型, 聊天",
  ],
  [
    "llm-role-translate",
    "role.translateModel",
    "advance.search.descs.llmRoleTranslate",
    "translate model, 翻译, 第二语言模型",
  ],
  [
    "llm-role-god",
    "role.godAgent",
    "advance.search.descs.llmRoleGod",
    "god agent, agent模型, 后台决策, 好感度评估",
  ],
  [
    "llm-role-vision",
    "role.visionModel",
    "advance.search.descs.llmRoleVision",
    "vision model, 识图, 多模态, 视觉理解, 看屏幕模型, 图片识别",
  ],
];

const TTS_DEVICE_ROW: AnchorRow = [
  "tts-device",
  "device.label",
  "CPU / GPU (DirectML) / GPU (WebGPU) / NPU (DirectML)",
  "cpu, gpu, directml, webgpu, npu, 显卡, 加速, inference device",
  { fallbackAnchor: "tts-switch" },
];

const TTS_ROWS: AnchorRow[] = [
  [
    "tts-switch",
    "switch.label",
    "advance.search.descs.ttsSwitch",
    "local tts, 离线语音, 本地tts, 语音合成开关",
  ],
  ["tts-deberta", "deberta.label", "deberta.warning", "deberta, tokenizer, 分词器, 本地模型状态"],
  ["tts-download", "download.title", "download.subtitle", "modelscope, 魔搭, 下载模型, download"],
  ["tts-import", "import.title", "import.subtitle", "sbv2, onnx, 导入模型, 导入音色, import"],
  [
    "tts-style-vectors",
    "styleVectors.title",
    "styleVectors.subtitle",
    "style_vectors, onnx, 风格向量",
    { fallbackAnchor: "tts-import" },
  ],
  [
    "tts-installed",
    "installed.title",
    "advance.search.descs.ttsInstalled",
    "角色语音, 声音模型, 已安装, voice list",
  ],
  ["tts-preview", "preview.title", "preview.subtitle", "语音试听, preview, length scale, 测试声音"],
  [
    "tts-cosyvoice",
    "cosyvoice.title",
    "cosyvoice.subtitle",
    "cosyvoice, dashscope, 声音克隆, 语音克隆, 云端音色, 百炼, clone",
  ],
];

const ASR_DESKTOP_ROWS: AnchorRow[] = [
  ["asr-ptt-key", "pttKey", "pttKeyHint", "ptt, 快捷键, 热键, hotkey, 按键说话, push to talk"],
  ["asr-ptt-global", "pttGlobal", "pttGlobalHint", "global hotkey, 后台按键, 全局热键"],
];

const TOOLS_GENERAL_ROWS: AnchorRow[] = [
  [
    "tools-general-admin",
    "adminModeTitle",
    "adminModeHint",
    "uac, 管理员, administrator, 以管理员身份重启",
    { fallbackAnchor: "tools-general-access-mode" },
  ],
  [
    "tools-general-max-rounds",
    "maxToolRoundsTitle",
    "maxToolRoundsHint",
    "max_tool_rounds, 轮数上限, tool rounds",
  ],
  ["tools-general-rag", "toolRagTitle", "toolRagHint", "tool rag, 按需检索, tool_search, 缓存容量"],
  [
    "tools-general-recommend",
    "toolRecommendTitle",
    "toolRecommendHint",
    "tool recommend, 工具推荐, 静默检索, agent",
  ],
];

const CAST_ROWS: AnchorRow[] = [
  ["cast-enable", "enable", "enableDesc", "投屏, 串流, cast, esp32, holocubic, mjpeg"],
  ["cast-port", "port", "portDesc", "端口, port, 投屏端口"],
  ["cast-resolution", "resolution", "resolutionDesc", "分辨率, resolution, 320x240, 尺寸"],
  ["cast-vivid", "vivid", "vividDesc", "vivid, 饱和度, 对比度, 小屏色彩"],
  ["cast-dialog-hide", "dialogHide.title", "dialogHide.desc", "隐藏对话框, 纯立绘投屏"],
  ["cast-character-adjust", "characterAdjust", "characterAdjustDesc", "投屏人物, 投屏缩放, 偏移"],
  ["cast-dialog-adjust", "dialogAdjust", "dialogAdjustDesc", "投屏对话框, 字号, 背景不透明度"],
  ["cast-connection", "connection", "smallScreenHint", "mjpeg, 预览地址, 局域网ip, 流地址"],
];

const MEMORY_DETAIL_ROWS: AnchorRow[] = [
  [
    "memory-compaction",
    "triggerTitle",
    "triggerHint",
    "压缩进度, 触发间隔, 保留窗口, memory trigger",
  ],
  ["memory-store", "storeTitle", "storeHint", "记忆真源, 长期事件, 用户画像, promises"],
  ["memory-injected", "injectedTitle", "injectedHint", "注入视图, system memory, 实际注入"],
  ["memory-context", "contextTitle", "contextEmpty", "上下文, messages, prompt上下文"],
];

export function useSettingsSearch(query: Ref<string>) {
  const { t, te, locale } = useI18n();
  const toolStore = useToolSettingsStore();
  const llmStore = useLlmProvidersStore();

  const hasKey = (key: string): boolean => te(key) || te(key, "zh-CN");
  const tr = (key: string, fallback = ""): string => (hasKey(key) ? t(key) : fallback);
  const textOf = (spec: string, prefix = ""): string => {
    const prefixed = prefix ? `${prefix}${spec}` : "";
    if (prefixed && hasKey(prefixed)) return t(prefixed);
    return hasKey(spec) ? t(spec) : spec;
  };

  const ensureCatalogData = async () => {
    const tasks: Array<Promise<unknown>> = [];

    if (!cachedEnvConfig.value) {
      if (!envConfigInFlight) {
        loadingEnvConfig.value = true;
        envConfigInFlight = getEnvConfigSettings()
          .then((data) => {
            cachedEnvConfig.value = data;
          })
          .catch(() => undefined)
          .finally(() => {
            loadingEnvConfig.value = false;
            envConfigInFlight = null;
          });
      }
      tasks.push(envConfigInFlight);
    }

    if (toolStore.orderedPackages.length === 0) tasks.push(toolStore.load().catch(() => undefined));
    if (!llmStore.loaded) tasks.push(llmStore.load().catch(() => undefined));
    if (tasks.length > 0) await Promise.all(tasks);
  };

  const catalog = computed<SettingsSearchItem[]>(() => {
    void locale.value;

    const mobile = isMobile();
    const navAdvance = tr("nav.advance", "高级设置");
    const sections: Record<AdvanceTab, string> = {
      menu: tr("advance.search.sections.menu", "主菜单"),
      llm: tr("advance.search.sections.llm", "大模型管理"),
      tts: tr("advance.search.sections.tts", "TTS 设置"),
      asr: tr("advance.search.sections.asr", "语音识别"),
      tools: tr("advance.search.sections.tools", "工具配置"),
      cast: tr("advance.search.sections.cast", "投屏设置"),
      memory: tr("advance.search.sections.memory", "永久记忆调试"),
      other: tr("advance.search.sections.other", "其他高级设置"),
    };

    const items: SettingsSearchItem[] = [];

    const add = (
      tab: AdvanceTab,
      id: string,
      title: string,
      description: string,
      aliases: string | string[],
      opts: AddItemOptions = {},
    ) => {
      const { crumbs, rawKey, restartRequired, ...targetFields } = opts;
      items.push({
        id,
        title,
        description,
        breadcrumbs: [navAdvance, sections[tab], ...(crumbs ?? [])],
        ...(rawKey ? { rawKey } : {}),
        ...(restartRequired ? { restartRequired } : {}),
        aliases: parseAliases(aliases),
        target: { advanceTab: tab, ...targetFields },
      });
    };

    const addAnchors = (
      tab: AdvanceTab,
      rows: AnchorRow[],
      keyPrefix = "",
      sharedOpts?: Omit<AddItemOptions, "anchor">,
    ) => {
      for (const [anchor, title, description, aliases, opts] of rows) {
        add(
          tab,
          `adv-${anchor}`,
          textOf(title, keyPrefix),
          textOf(description, keyPrefix),
          aliases,
          {
            ...sharedOpts,
            ...opts,
            anchor,
          },
        );
      }
    };

    // 主菜单、大模型、TTS
    addAnchors("menu", MENU_ROWS, "advance.");
    addAnchors("llm", LLM_ROWS, "settings.llmProviders.");

    const llmListTitle = tr("settings.llmProviders.list.title", "已配置模型");
    for (const p of llmStore.providers) {
      add(
        "llm",
        `adv-llm-provider-${p.id}`,
        p.label || tr("settings.llmProviders.list.unnamed", "(未命名)"),
        [p.provider, p.model].filter(Boolean).join(" · "),
        [p.label, p.model, p.provider, p.base_url || "", "llm", "model", "模型"],
        {
          crumbs: [llmListTitle],
          rawKey: p.model || p.provider,
          anchor: `llm-provider-${p.id}`,
          fallbackAnchor: "llm-list",
        },
      );
    }

    addAnchors(
      "tts",
      !mobile && !isMacOS() ? [TTS_ROWS[0], TTS_DEVICE_ROW, ...TTS_ROWS.slice(1)] : TTS_ROWS,
      "settings.tts.",
    );

    // 语音识别 (ASR)
    const asrInputMethod = tr("settings.asr.sections.inputMethod", "输入方式");
    addAnchors(
      "asr",
      [
        [
          "asr-voice-input",
          "voiceInput",
          mobile ? "voiceInputHintMobile" : "voiceInputHint",
          "asr, 麦克风, 语音识别, speech recognition, 语音开关",
        ],
        [
          "asr-input-method",
          asrInputMethod,
          [
            !mobile ? tr("settings.asr.pttKey") : "",
            tr("settings.asr.autoListen"),
            tr("settings.asr.sendMode.title"),
          ]
            .filter(Boolean)
            .join(" · "),
          "ptt, 按住说话, 自动监听, 发送方式, vad, 快捷键, hotkey",
        ],
        [
          "asr-provider",
          "provider.title",
          `${tr("settings.asr.provider.providerSelect")} · ${tr("settings.asr.streamMode")}`,
          "阿里云, asr服务, 流式识别, streaming, 测试连接, 模型选择",
        ],
        [
          "asr-hotwords",
          "hotwords.sectionTitle",
          "hotwords.description",
          "全局热词, 热词词库, hotword, 专有名词",
        ],
        [
          "asr-advanced",
          "advanced.title",
          [
            tr("settings.asr.vadSilence"),
            tr("settings.asr.energyWarmup"),
            tr("settings.asr.vadDebugLog"),
          ].join(" · "),
          "vad, 静音等待, 能量预热, vad日志, diagnostics",
        ],
      ],
      "settings.asr.",
    );
    addAnchors(
      "asr",
      [
        ...(!mobile ? ASR_DESKTOP_ROWS : []),
        [
          "asr-auto-listen",
          "autoListen",
          "autoListenHint",
          "auto listen, 免提, 环境监听, 自动录音, vad",
        ],
        [
          "asr-send-mode",
          "sendMode.title",
          `${tr("settings.asr.sendMode.fillOnly")} / ${tr("settings.asr.sendMode.autoSend")}`,
          "自动发送, 填充输入框, send mode",
        ],
      ],
      "settings.asr.",
      { crumbs: [asrInputMethod], fallbackAnchor: "asr-input-method" },
    );

    // 工具配置
    const toolsGeneral = tr("ui.toolCalls.globalSettingsTitle", "其它高级设置");
    add(
      "tools",
      "adv-tools-general",
      toolsGeneral,
      tr("ui.toolCalls.globalSettingsHint"),
      "工具全局设置, 工具权限, access mode, tool rag, agent, 工具轮数",
      { toolGroup: "general", openToolSubpage: true, anchor: "tools-subpage-panel" },
    );
    addAnchors(
      "tools",
      [
        [
          "tools-general-access-mode",
          "accessModes.manual.label",
          [
            tr("ui.toolCalls.accessModes.manual.label"),
            tr("ui.toolCalls.accessModes.auto_approve.label"),
            tr("ui.toolCalls.accessModes.full_access.label"),
          ].join(" / "),
          "沙盒, sandbox, 全权限, full_access, auto_approve, 文件审批, 命令审批",
        ],
        ...TOOLS_GENERAL_ROWS,
      ],
      "ui.toolCalls.",
      { crumbs: [toolsGeneral], toolGroup: "general", openToolSubpage: true },
    );

    const addToolSubpage = (
      id: string,
      toolGroup: string,
      title: string,
      desc: string,
      aliases: string,
      crumb?: string,
      anchor = "tools-subpage-panel",
    ) =>
      add("tools", id, textOf(title, "ui.toolCalls."), textOf(desc, "ui.toolCalls."), aliases, {
        ...(crumb ? { crumbs: [crumb] } : {}),
        toolGroup,
        openToolSubpage: true,
        anchor,
      });

    addToolSubpage(
      "adv-tools-web-search-subpage",
      "web_search",
      `${tr("ui.toolCalls.webSearchTitle")} · ${tr("ui.toolCalls.provider")}`,
      "advance.search.descs.toolsWebSearch",
      "联网搜索, web search, keenable, kimi, bocha, 博查, tavily, proxy, 代理, 搜索api",
      tr("ui.toolCalls.webSearchTitle"),
    );
    addToolSubpage(
      "adv-tools-memory-embedding",
      "memory",
      "embedding.cloudToggle",
      "embedding.cloudToggleHint",
      "embedding, 向量, 云端向量, bge-m3, gemini, openai, 重算向量, 笔记检索",
      tr("ui.toolCalls.nav.memory"),
    );
    addToolSubpage(
      "adv-tools-media-subpage",
      "media",
      "mediaTitle",
      "mediaHint",
      "readmediafile, 图片识别, 视频识别, 压缩边长, jpeg质量, 视觉工具",
      tr("ui.toolCalls.mediaTitle"),
    );
    addToolSubpage(
      "adv-tools-reset-all",
      "general",
      "resetAll",
      "resetAllHint",
      "重置工具, 关闭全部工具, reset tools",
      undefined,
      "tools-reset-all",
    );

    for (const pack of toolStore.orderedPackages) {
      const packName = tr(`ui.toolCalls.nav.${pack.group}`, pack.group);
      const packAnchor = `tool-pack-${pack.group}`;
      add(
        "tools",
        `adv-tool-pack-${pack.group}`,
        packName,
        tr(`ui.toolCalls.groups.${pack.group}`),
        [pack.group, packName, "工具包", "tool pack"],
        { rawKey: pack.group, toolGroup: pack.group, openToolSubpage: false, anchor: packAnchor },
      );

      for (const toolName of pack.tools) {
        const displayName = toolDisplayName(toolName);
        add(
          "tools",
          `adv-tool-item-${pack.group}-${toolName}`,
          displayName,
          toolDescription(toolName),
          [toolName, displayName, pack.group, packName],
          {
            crumbs: [packName],
            rawKey: toolName,
            toolGroup: pack.group,
            openToolSubpage: false,
            anchor: `tool-row-${toolName}`,
            fallbackAnchor: packAnchor,
          },
        );
      }
    }

    // 投屏设置 (Cast)
    addAnchors(
      "cast",
      [
        ...CAST_ROWS,
        [
          "cast-performance",
          "performance",
          `${tr("settings.cast.fps")} · ${tr("settings.cast.quality")}`,
          "帧率, fps, 画质, quality, 带宽",
        ],
        [
          "cast-control",
          "control",
          `${tr("settings.cast.openWindow")} / ${tr("settings.cast.start")}`,
          "投屏窗口, 开始串流, 停止串流",
        ],
      ],
      "settings.cast.",
    );

    // 永久记忆调试
    addAnchors(
      "memory",
      [
        [
          "memory-overview",
          "title",
          "selectRoleHint",
          "memory debug, 记忆调试, 记忆库, 压缩指针, 真实上下文",
        ],
      ],
      "settings.memoryDebug.",
    );
    addAnchors("memory", MEMORY_DETAIL_ROWS, "settings.memoryDebug.", {
      fallbackAnchor: "memory-overview",
    });

    // 其他高级设置（动态配置树 + 附加区块）
    const configData = cachedEnvConfig.value;
    let ttsFirstSubcategory = "适配器 URL";

    if (configData) {
      for (const [catKey, catData] of Object.entries(configData)) {
        const catTitle = tr(`settings.advanceOther.categories.${catKey}`, catKey);
        const subEntries = Object.entries(catData.subcategories ?? {});
        if (catKey === "TTS 配置" && subEntries.length > 0) ttsFirstSubcategory = subEntries[0][0];

        for (const [subKey, subData] of subEntries) {
          const subTitle = tr(`settings.advanceOther.subcategories.${subKey}`, subKey);
          const subDesc = hasKey(`settings.advanceOther.subcategoryDescs.${subKey}`)
            ? t(`settings.advanceOther.subcategoryDescs.${subKey}`)
            : t("settings.advanceOther.subcategoryDesc", { name: subTitle });

          add(
            "other",
            `adv-other-sub-${catKey}-${subKey}`,
            subTitle,
            subDesc,
            [catKey, subKey, catTitle, subTitle],
            { crumbs: [catTitle], category: catKey, subcategory: subKey },
          );

          for (const setting of subData.settings ?? []) {
            const fieldLabel = tr(
              `settings.advanceOther.fields.${setting.key}`,
              setting.description || setting.key,
            );
            const valPreview = formatEnvValuePreview(setting.key, setting.value);
            add(
              "other",
              `adv-other-key-${catKey}-${subKey}-${setting.key}`,
              fieldLabel,
              valPreview ? `${subTitle} · ${valPreview}` : subDesc,
              [
                catKey,
                catTitle,
                subKey,
                subTitle,
                setting.key,
                setting.description || "",
                ...(ENV_KEY_ALIASES[setting.key] ?? []),
              ],
              {
                crumbs: [catTitle, subTitle],
                rawKey: setting.key,
                restartRequired: catKey !== "好感度" && setting.key !== "log.genai_debug",
                category: catKey,
                subcategory: subKey,
                settingKey: setting.key,
              },
            );
          }
        }
      }
    }

    add(
      "other",
      "adv-other-tts-control",
      tr("settings.advanceOther.ttsControl.title", "TTS 连接控制"),
      tr("settings.advanceOther.ttsControl.desc"),
      "重连tts, 强制重连, tts离线, force reconnect, tts control",
      {
        crumbs: [tr("settings.advanceOther.categories.TTS 配置", "TTS 配置")],
        category: "TTS 配置",
        subcategory: ttsFirstSubcategory,
        anchor: "tts-control",
      },
    );
    add(
      "other",
      "adv-other-affection-anim",
      tr("settings.advanceOther.affectionAnim.title", "好感度动画"),
      tr("settings.advanceOther.affectionAnim.desc"),
      "心跳, 波浪, 心形, 好感度动画, heartbeat, wave, 即时生效",
      {
        crumbs: [
          tr("settings.advanceOther.categories.好感度", "好感度"),
          tr("settings.advanceOther.subcategories.通用", "通用"),
        ],
        category: "好感度",
        subcategory: "通用",
        anchor: "affection-anim",
      },
    );

    return items;
  });

  const allMatches = computed<SettingsSearchMatch[]>(() => {
    const tokens = query.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
    if (tokens.length === 0) return [];

    const scored: SettingsSearchMatch[] = [];
    for (const item of catalog.value) {
      const titleLower = item.title.toLowerCase();
      const rawKeyLower = (item.rawKey ?? "").toLowerCase();
      const aliasesLower = item.aliases.map((a) => a.toLowerCase());
      const breadcrumbLower = item.breadcrumbs.join(" ").toLowerCase();
      const descLower = item.description.toLowerCase();

      let totalScore = 0;
      let matchedAll = true;
      for (const token of tokens) {
        const s = scoreToken(
          token,
          titleLower,
          rawKeyLower,
          aliasesLower,
          breadcrumbLower,
          descLower,
        );
        if (s === 0) {
          matchedAll = false;
          break;
        }
        totalScore += s;
      }

      if (!matchedAll) continue;
      if (item.rawKey || item.target.settingKey || item.target.anchor?.includes("-")) {
        totalScore += 2;
      }

      scored.push({ item, score: totalScore });
    }

    scored.sort(
      (a, b) =>
        b.score - a.score ||
        a.item.title.length - b.item.title.length ||
        a.item.id.localeCompare(b.item.id),
    );

    return scored;
  });

  const results = computed(() => allMatches.value.slice(0, MAX_RENDERED_RESULTS));
  const totalMatches = computed(() => allMatches.value.length);

  return {
    catalog,
    results,
    totalMatches,
    loadingDynamic: loadingEnvConfig,
    ensureCatalogData,
  };
}
