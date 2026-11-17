import { computed, ref, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { getEnvConfigSettings, type ConfigItem } from "@/api/services/config";
import { toolDescription, toolDisplayName } from "@/api/services/tool-settings";
import { useLlmProvidersStore } from "@/stores/modules/llm-providers";
import { useToolSettingsStore } from "@/stores/modules/settings/tools";
import type { SettingsJumpTarget } from "@/stores/modules/ui/ui";
import { isAndroid, isMobile, isWindows } from "@/utils/platform";

export type SettingsSearchTarget = Omit<SettingsJumpTarget, "nonce" | "crossTab">;

export interface SettingsSearchItem {
  id: string;
  title: string;
  description: string;
  breadcrumbs: string[];
  badge?: string;
  rawKey?: string;
  restartRequired?: boolean;
  aliases: string[];
  isAdvance: boolean;
  target: SettingsSearchTarget;
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

// 缓存后端配置树，避免每次回到主菜单都重复拉取
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

function mergeRegexRanges(ranges: Array<[number, number]>): Array<[number, number]> {
  if (ranges.length === 0) return [];
  const sorted = [...ranges].sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const merged: Array<[number, number]> = [sorted[0]];
  for (let i = 1; i < sorted.length; i += 1) {
    const prev = merged[merged.length - 1];
    const curr = sorted[i];
    if (curr[0] <= prev[1]) {
      prev[1] = Math.max(prev[1], curr[1]);
    } else {
      merged.push(curr);
    }
  }
  return merged;
}

/** 将文本拆分为普通片段与关键词高亮片段（忽略大小写，支持多词高亮）。 */
export function splitHighlightSegments(text: string, query: string): HighlightSegment[] {
  if (!text) return [];
  const tokens = query
    .trim()
    .split(/\s+/)
    .map((token) => token.trim())
    .filter(Boolean);
  if (tokens.length === 0) {
    return [{ text, matched: false }];
  }

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

  const merged = mergeRegexRanges(ranges);
  if (merged.length === 0) {
    return [{ text, matched: false }];
  }

  const segments: HighlightSegment[] = [];
  let cursor = 0;
  for (const [start, end] of merged) {
    if (start > cursor) {
      segments.push({ text: text.slice(cursor, start), matched: false });
    }
    segments.push({ text: text.slice(start, end), matched: true });
    cursor = end;
  }
  if (cursor < text.length) {
    segments.push({ text: text.slice(cursor), matched: false });
  }
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
    if (titleIdx >= 0) {
      best = Math.max(best, Math.max(52, 74 - Math.min(titleIdx, 20)));
    }
    if (rawKeyLower.includes(token)) {
      best = Math.max(best, 58);
    }
  }

  for (const alias of aliasesLower) {
    if (alias === token) {
      best = Math.max(best, 56);
    } else if (alias.startsWith(token)) {
      best = Math.max(best, 44);
    } else if (alias.includes(token)) {
      best = Math.max(best, 32);
    }
  }

  if (breadcrumbLower.includes(token)) {
    best = Math.max(best, 22);
  }
  if (descLower.includes(token)) {
    best = Math.max(best, 15);
  }

  return best;
}

function formatEnvValuePreview(key: string, value: unknown): string {
  if (SENSITIVE_KEY_RE.test(key)) return "";
  if (typeof value === "boolean") return value ? "ON" : "OFF";
  if (typeof value === "number") return String(value);
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!trimmed) return "";
    return trimmed.length > 36 ? `${trimmed.slice(0, 36)}…` : trimmed;
  }
  return "";
}

export function useSettingsSearch(query: Ref<string>) {
  const { t, te, locale } = useI18n();
  const toolStore = useToolSettingsStore();
  const llmStore = useLlmProvidersStore();

  const tr = (key: string, fallback = ""): string => (te(key) ? t(key) : fallback);

  const ensureCatalogData = async () => {
    const tasks: Array<Promise<unknown>> = [];

    if (!cachedEnvConfig.value) {
      if (!envConfigInFlight) {
        loadingEnvConfig.value = true;
        envConfigInFlight = getEnvConfigSettings()
          .then((data) => {
            cachedEnvConfig.value = data;
          })
          .catch(() => {
            // 动态配置加载失败时降级保留静态设置目录
          })
          .finally(() => {
            loadingEnvConfig.value = false;
            envConfigInFlight = null;
          });
      }
      tasks.push(envConfigInFlight);
    }

    if (toolStore.orderedPackages.length === 0) {
      tasks.push(toolStore.load().catch(() => undefined));
    }
    if (!llmStore.loaded) {
      tasks.push(llmStore.load().catch(() => undefined));
    }

    if (tasks.length > 0) {
      await Promise.all(tasks);
    }
  };

  const catalog = computed<SettingsSearchItem[]>(() => {
    // 显式读取 locale，确保切换界面语言后搜索索引即时刷新
    void locale.value;

    const mobile = isMobile();
    const android = isAndroid();
    const windows = isWindows();

    const navAdvance = tr("nav.advance", "高级设置");
    const tabMenu = tr("advance.search.sections.menu", "主菜单");
    const tabLlm = tr("advance.search.sections.llm", "大模型管理");
    const tabTts = tr("advance.search.sections.tts", "TTS 设置");
    const tabAsr = tr("advance.search.sections.asr", "语音识别");
    const tabTools = tr("advance.search.sections.tools", "工具配置");
    const tabCast = tr("advance.search.sections.cast", "投屏设置");
    const tabMemory = tr("advance.search.sections.memory", "永久记忆调试");
    const tabOther = tr("advance.search.sections.other", "其他高级设置");

    const items: SettingsSearchItem[] = [];

    // -----------------------------------------------------------------------
    // 1. 高级设置 -> 主菜单原地卡片
    // -----------------------------------------------------------------------
    items.push(
      {
        id: "adv-menu-language",
        title: tr("advance.menu.languageTitle", "界面语言"),
        description: tr(
          "advance.search.descs.menuLanguage",
          "切换应用界面显示语言，立即生效并自动保存",
        ),
        breadcrumbs: [navAdvance, tabMenu],
        aliases: [
          "language",
          "locale",
          "i18n",
          "语言",
          "简体中文",
          "繁体中文",
          "english",
          "日本語",
        ],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "menu", anchor: "menu-language" },
      },
      {
        id: "adv-menu-affection",
        title: tr("advance.menu.affectionTitle", "好感度系统"),
        description: tr("advance.search.descs.menuAffection", "好感度系统总开关，保存后立即生效"),
        breadcrumbs: [navAdvance, tabMenu],
        aliases: ["好感", "affection", "总开关", "好感度开关"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "menu", anchor: "menu-affection" },
      },
      {
        id: "adv-menu-guide",
        title: tr("advance.menu.guideTitle", "内置 TTS 教程"),
        description: tr("advance.search.descs.menuGuide", "查看内置 TTS 配置与使用指南"),
        breadcrumbs: [navAdvance, tabMenu],
        aliases: ["教程", "指南", "guide", "tutorial", "tts教程"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "menu", anchor: "menu-guide" },
      },
    );

    // -----------------------------------------------------------------------
    // 2. 高级设置 -> 大模型管理 (静态区块 + 已配置模型)
    // -----------------------------------------------------------------------
    items.push(
      {
        id: "adv-llm-list",
        title: tr("settings.llmProviders.list.title", tabLlm),
        description: tr(
          "advance.search.descs.llmList",
          "查看、添加、编辑、删除并测试已配置的 AI 大模型连接与参数",
        ),
        breadcrumbs: [navAdvance, tabLlm],
        aliases: [
          "llm",
          "api",
          "openai",
          "deepseek",
          "codex",
          "lm studio",
          "大模型",
          "模型管理",
          "api key",
          "base url",
          "温度",
          "temperature",
        ],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "llm", anchor: "llm-list" },
      },
      {
        id: "adv-llm-add",
        title: tr("settings.llmProviders.list.addModel", "添加模型"),
        description: tr(
          "advance.search.descs.llmAdd",
          "新建 OpenAI 兼容 / LM Studio / Codex 等模型连接配置",
        ),
        breadcrumbs: [navAdvance, tabLlm],
        aliases: ["add model", "新增模型", "添加api", "预设", "preset", "接入模型"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "llm", anchor: "llm-add" },
      },
      {
        id: "adv-llm-role-chat",
        title: tr("settings.llmProviders.role.chatModel", "聊天模型"),
        description: tr("advance.search.descs.llmRoleChat", "指定主对话默认使用的 AI 模型提供方"),
        breadcrumbs: [navAdvance, tabLlm],
        aliases: ["chat model", "对话模型", "主模型", "聊天"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "llm", anchor: "llm-role-chat" },
      },
      {
        id: "adv-llm-role-translate",
        title: tr("settings.llmProviders.role.translateModel", "翻译模型"),
        description: tr(
          "advance.search.descs.llmRoleTranslate",
          "指定用于双语台词翻译的模型提供方",
        ),
        breadcrumbs: [navAdvance, tabLlm],
        aliases: ["translate model", "翻译", "第二语言模型"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "llm", anchor: "llm-role-translate" },
      },
      {
        id: "adv-llm-role-god",
        title: tr("settings.llmProviders.role.godAgent", "上帝 Agent"),
        description: tr(
          "advance.search.descs.llmRoleGod",
          "指定用于后台判断与工具推荐的轻量决策模型",
        ),
        breadcrumbs: [navAdvance, tabLlm],
        aliases: ["god agent", "agent模型", "后台决策", "好感度评估"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "llm", anchor: "llm-role-god" },
      },
      {
        id: "adv-llm-role-vision",
        title: tr("settings.llmProviders.role.visionModel", "视觉模型"),
        description: tr(
          "advance.search.descs.llmRoleVision",
          "指定用于看屏幕、图片与视频理解的多模态模型",
        ),
        breadcrumbs: [navAdvance, tabLlm],
        aliases: ["vision model", "识图", "多模态", "视觉理解", "看屏幕模型", "图片识别"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "llm", anchor: "llm-role-vision" },
      },
    );

    for (const provider of llmStore.providers) {
      items.push({
        id: `adv-llm-provider-${provider.id}`,
        title: provider.label || tr("settings.llmProviders.list.unnamed", "(未命名)"),
        description: [provider.provider, provider.model].filter(Boolean).join(" · "),
        breadcrumbs: [navAdvance, tabLlm, tr("settings.llmProviders.list.title", "已配置模型")],
        rawKey: provider.model || provider.provider,
        aliases: [
          provider.label,
          provider.model,
          provider.provider,
          provider.base_url || "",
          "llm",
          "model",
          "模型",
        ].filter(Boolean),
        isAdvance: true,
        target: {
          settingsTab: "advance",
          advanceTab: "llm",
          anchor: `llm-provider-${provider.id}`,
          llmProviderId: provider.id,
        },
      });
    }

    // -----------------------------------------------------------------------
    // 3. 高级设置 -> TTS 设置
    // -----------------------------------------------------------------------
    items.push(
      {
        id: "adv-tts-switch",
        title: tr("settings.tts.switch.label", "全局本地 TTS"),
        description: tr("advance.search.descs.ttsSwitch", "启用或关闭离线本地 TTS 引擎"),
        breadcrumbs: [navAdvance, tabTts],
        aliases: ["local tts", "离线语音", "本地tts", "语音合成开关"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "tts", anchor: "tts-switch" },
      },
      {
        id: "adv-tts-device",
        title: tr("settings.tts.device.label", "推理设备"),
        description: "CPU / GPU (DirectML) / GPU (WebGPU) / NPU (DirectML)",
        breadcrumbs: [navAdvance, tabTts],
        aliases: ["cpu", "gpu", "directml", "webgpu", "npu", "显卡", "加速", "inference device"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "tts", anchor: "tts-device" },
      },
      {
        id: "adv-tts-deberta",
        title: tr("settings.tts.deberta.label", "DeBERTa 与分词器"),
        description: tr("settings.tts.deberta.warning", "本地 TTS 依赖模型状态检查与管理"),
        breadcrumbs: [navAdvance, tabTts],
        aliases: ["deberta", "tokenizer", "分词器", "本地模型状态"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "tts", anchor: "tts-deberta" },
      },
      {
        id: "adv-tts-download",
        title: tr("settings.tts.download.title", "模型下载"),
        description: tr("settings.tts.download.subtitle", "一键从 ModelScope 获取本地 TTS 模型"),
        breadcrumbs: [navAdvance, tabTts],
        aliases: ["modelscope", "魔搭", "下载模型", "download"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "tts", anchor: "tts-download" },
      },
      {
        id: "adv-tts-import",
        title: tr("settings.tts.import.title", "本地导入"),
        description: tr("settings.tts.import.subtitle", "支持导入 .sbv2 / .onnx 模型文件"),
        breadcrumbs: [navAdvance, tabTts],
        aliases: ["sbv2", "onnx", "导入模型", "导入音色", "import"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "tts", anchor: "tts-import" },
      },
      {
        id: "adv-tts-style-vectors",
        title: tr("settings.tts.styleVectors.title", "补全 style_vectors"),
        description: tr(
          "settings.tts.styleVectors.subtitle",
          "为 .onnx 语音模型补全 style_vectors.json",
        ),
        breadcrumbs: [navAdvance, tabTts],
        aliases: ["style_vectors", "onnx", "风格向量"],
        isAdvance: true,
        target: {
          settingsTab: "advance",
          advanceTab: "tts",
          anchor: "tts-style-vectors",
          fallbackAnchor: "tts-import",
        },
      },
      {
        id: "adv-tts-installed",
        title: tr("settings.tts.installed.title", "已安装语音"),
        description: tr("advance.search.descs.ttsInstalled", "管理已安装的本地角色语音模型"),
        breadcrumbs: [navAdvance, tabTts],
        aliases: ["角色语音", "声音模型", "已安装", "voice list"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "tts", anchor: "tts-installed" },
      },
      {
        id: "adv-tts-preview",
        title: tr("settings.tts.preview.title", "试听"),
        description: tr("settings.tts.preview.subtitle", "输入文本试听本地角色语音模型效果"),
        breadcrumbs: [navAdvance, tabTts],
        aliases: ["语音试听", "preview", "length scale", "测试声音"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "tts", anchor: "tts-preview" },
      },
      {
        id: "adv-tts-cosyvoice",
        title: tr("settings.tts.cosyvoice.title", "语音克隆 TTS"),
        description: tr(
          "settings.tts.cosyvoice.subtitle",
          "阿里云百炼 CosyVoice 云端声音克隆与音色注册",
        ),
        breadcrumbs: [navAdvance, tabTts],
        aliases: ["cosyvoice", "dashscope", "声音克隆", "语音克隆", "云端音色", "百炼", "clone"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "tts", anchor: "tts-cosyvoice" },
      },
    );

    // -----------------------------------------------------------------------
    // 4. 高级设置 -> 语音识别 (ASR)
    // -----------------------------------------------------------------------
    items.push(
      {
        id: "adv-asr-voice-input",
        title: tr("settings.asr.voiceInput", "语音输入"),
        description: mobile
          ? tr("settings.asr.voiceInputHintMobile", "")
          : tr("settings.asr.voiceInputHint", ""),
        breadcrumbs: [navAdvance, tabAsr],
        aliases: ["asr", "麦克风", "语音识别", "speech recognition", "语音开关"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "asr", anchor: "asr-voice-input" },
      },
      {
        id: "adv-asr-input-method",
        title: tr("settings.asr.sections.inputMethod", "输入方式"),
        description: [
          !mobile ? tr("settings.asr.pttKey", "按住说话快捷键") : "",
          tr("settings.asr.autoListen", "自动监听"),
          tr("settings.asr.sendMode.title", "识别后"),
        ]
          .filter(Boolean)
          .join(" · "),
        breadcrumbs: [navAdvance, tabAsr],
        aliases: ["ptt", "按住说话", "自动监听", "发送方式", "vad", "快捷键", "hotkey"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "asr", anchor: "asr-input-method" },
      },
      ...(!mobile
        ? [
            {
              id: "adv-asr-ptt-key",
              title: tr("settings.asr.pttKey", "按住说话快捷键"),
              description: tr("settings.asr.pttKeyHint", ""),
              breadcrumbs: [
                navAdvance,
                tabAsr,
                tr("settings.asr.sections.inputMethod", "输入方式"),
              ],
              aliases: ["ptt", "快捷键", "热键", "hotkey", "按键说话", "push to talk"],
              isAdvance: true,
              target: {
                settingsTab: "advance",
                advanceTab: "asr",
                anchor: "asr-ptt-key",
                fallbackAnchor: "asr-input-method",
              },
            },
            {
              id: "adv-asr-ptt-global",
              title: tr("settings.asr.pttGlobal", "全局快捷键"),
              description: tr("settings.asr.pttGlobalHint", ""),
              breadcrumbs: [
                navAdvance,
                tabAsr,
                tr("settings.asr.sections.inputMethod", "输入方式"),
              ],
              aliases: ["global hotkey", "后台按键", "全局热键"],
              isAdvance: true,
              target: {
                settingsTab: "advance",
                advanceTab: "asr",
                anchor: "asr-ptt-global",
                fallbackAnchor: "asr-input-method",
              },
            },
          ]
        : []),
      {
        id: "adv-asr-auto-listen",
        title: tr("settings.asr.autoListen", "自动监听"),
        description: tr("settings.asr.autoListenHint", ""),
        breadcrumbs: [navAdvance, tabAsr, tr("settings.asr.sections.inputMethod", "输入方式")],
        aliases: ["auto listen", "免提", "环境监听", "自动录音", "vad"],
        isAdvance: true,
        target: {
          settingsTab: "advance",
          advanceTab: "asr",
          anchor: "asr-auto-listen",
          fallbackAnchor: "asr-input-method",
        },
      },
      {
        id: "adv-asr-send-mode",
        title: tr("settings.asr.sendMode.title", "识别后"),
        description: `${tr("settings.asr.sendMode.fillOnly", "仅填充输入框")} / ${tr("settings.asr.sendMode.autoSend", "直接发送")}`,
        breadcrumbs: [navAdvance, tabAsr, tr("settings.asr.sections.inputMethod", "输入方式")],
        aliases: ["自动发送", "填充输入框", "send mode"],
        isAdvance: true,
        target: {
          settingsTab: "advance",
          advanceTab: "asr",
          anchor: "asr-send-mode",
          fallbackAnchor: "asr-input-method",
        },
      },
      {
        id: "adv-asr-provider",
        title: tr("settings.asr.provider.title", "识别服务"),
        description: `${tr("settings.asr.provider.providerSelect", "服务")} · ${tr("settings.asr.streamMode", "流式识别")}`,
        breadcrumbs: [navAdvance, tabAsr],
        aliases: ["阿里云", "asr服务", "流式识别", "streaming", "测试连接", "模型选择"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "asr", anchor: "asr-provider" },
      },
      {
        id: "adv-asr-hotwords",
        title: tr("settings.asr.hotwords.sectionTitle", "热词"),
        description: tr("settings.asr.hotwords.description", ""),
        breadcrumbs: [navAdvance, tabAsr],
        aliases: ["全局热词", "热词词库", "hotword", "专有名词"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "asr", anchor: "asr-hotwords" },
      },
      {
        id: "adv-asr-advanced",
        title: tr("settings.asr.advanced.title", "高级与诊断"),
        description: [
          tr("settings.asr.vadSilence", "静音结束等待"),
          tr("settings.asr.energyWarmup", "能量监听预热"),
          tr("settings.asr.vadDebugLog", "VAD 详细日志"),
        ].join(" · "),
        breadcrumbs: [navAdvance, tabAsr],
        aliases: ["vad", "静音等待", "能量预热", "vad日志", "diagnostics"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "asr", anchor: "asr-advanced" },
      },
    );

    // -----------------------------------------------------------------------
    // 5. 高级设置 -> 工具配置（总览 + 子页 + 动态工具包/工具）
    // -----------------------------------------------------------------------
    items.push(
      {
        id: "adv-tools-general",
        title: tr("ui.toolCalls.globalSettingsTitle", "其它高级设置"),
        description: tr(
          "ui.toolCalls.globalSettingsHint",
          "权限模式、调用轮数、按需检索与工具建议",
        ),
        breadcrumbs: [navAdvance, tabTools],
        aliases: ["工具全局设置", "工具权限", "access mode", "tool rag", "agent", "工具轮数"],
        isAdvance: true,
        target: {
          settingsTab: "advance",
          advanceTab: "tools",
          toolGroup: "general",
          openToolSubpage: true,
        },
      },
      {
        id: "adv-tools-general-access-mode",
        title: tr("ui.toolCalls.accessModes.manual.label", "访问与审批模式"),
        description: [
          tr("ui.toolCalls.accessModes.manual.label", "每次询问"),
          tr("ui.toolCalls.accessModes.auto_approve.label", "自动批准"),
          tr("ui.toolCalls.accessModes.full_access.label", "完全访问"),
        ].join(" / "),
        breadcrumbs: [navAdvance, tabTools, tr("ui.toolCalls.globalSettingsTitle", "其它高级设置")],
        aliases: [
          "沙盒",
          "sandbox",
          "全权限",
          "full_access",
          "auto_approve",
          "文件审批",
          "命令审批",
        ],
        isAdvance: true,
        target: {
          settingsTab: "advance",
          advanceTab: "tools",
          toolGroup: "general",
          openToolSubpage: true,
          anchor: "tools-general-access-mode",
        },
      },
      {
        id: "adv-tools-general-admin",
        title: tr("ui.toolCalls.adminModeTitle", "管理员权限状态"),
        description: tr("ui.toolCalls.adminModeHint", "以管理员身份重启以继承 UAC 令牌"),
        breadcrumbs: [navAdvance, tabTools, tr("ui.toolCalls.globalSettingsTitle", "其它高级设置")],
        aliases: ["uac", "管理员", "administrator", "以管理员身份重启"],
        isAdvance: true,
        target: {
          settingsTab: "advance",
          advanceTab: "tools",
          toolGroup: "general",
          openToolSubpage: true,
          anchor: "tools-general-admin",
          fallbackAnchor: "tools-general-access-mode",
        },
      },
      {
        id: "adv-tools-general-max-rounds",
        title: tr("ui.toolCalls.maxToolRoundsTitle", "单次回复工具调用轮数上限"),
        description: tr("ui.toolCalls.maxToolRoundsHint", ""),
        breadcrumbs: [navAdvance, tabTools, tr("ui.toolCalls.globalSettingsTitle", "其它高级设置")],
        aliases: ["max_tool_rounds", "轮数上限", "tool rounds"],
        isAdvance: true,
        target: {
          settingsTab: "advance",
          advanceTab: "tools",
          toolGroup: "general",
          openToolSubpage: true,
          anchor: "tools-general-max-rounds",
        },
      },
      {
        id: "adv-tools-general-rag",
        title: tr("ui.toolCalls.toolRagTitle", "按需检索工具"),
        description: tr("ui.toolCalls.toolRagHint", ""),
        breadcrumbs: [navAdvance, tabTools, tr("ui.toolCalls.globalSettingsTitle", "其它高级设置")],
        aliases: ["tool rag", "按需检索", "tool_search", "缓存容量"],
        isAdvance: true,
        target: {
          settingsTab: "advance",
          advanceTab: "tools",
          toolGroup: "general",
          openToolSubpage: true,
          anchor: "tools-general-rag",
        },
      },
      {
        id: "adv-tools-general-recommend",
        title: tr("ui.toolCalls.toolRecommendTitle", "工具建议方式"),
        description: tr("ui.toolCalls.toolRecommendHint", ""),
        breadcrumbs: [navAdvance, tabTools, tr("ui.toolCalls.globalSettingsTitle", "其它高级设置")],
        aliases: ["tool recommend", "工具推荐", "静默检索", "agent"],
        isAdvance: true,
        target: {
          settingsTab: "advance",
          advanceTab: "tools",
          toolGroup: "general",
          openToolSubpage: true,
          anchor: "tools-general-recommend",
        },
      },
      {
        id: "adv-tools-web-search-subpage",
        title: `${tr("ui.toolCalls.webSearchTitle", "网页搜索")} · ${tr("ui.toolCalls.provider", "搜索服务商")}`,
        description: tr(
          "advance.search.descs.toolsWebSearch",
          "配置 Keenable / Kimi / 博查 / DeepSeek / Tavily / Codex / 代理地址等联网搜索参数",
        ),
        breadcrumbs: [navAdvance, tabTools, tr("ui.toolCalls.webSearchTitle", "网页搜索")],
        aliases: [
          "联网搜索",
          "web search",
          "keenable",
          "kimi",
          "bocha",
          "博查",
          "tavily",
          "proxy",
          "代理",
          "搜索api",
        ],
        isAdvance: true,
        target: {
          settingsTab: "advance",
          advanceTab: "tools",
          toolGroup: "web_search",
          openToolSubpage: true,
          anchor: "tools-subpage-panel",
        },
      },
      {
        id: "adv-tools-memory-embedding",
        title: tr("ui.toolCalls.embedding.cloudToggle", "笔记语义搜索与云端向量化"),
        description: tr("ui.toolCalls.embedding.cloudToggleHint", ""),
        breadcrumbs: [navAdvance, tabTools, tr("ui.toolCalls.nav.memory", "笔记与记忆")],
        aliases: [
          "embedding",
          "向量",
          "云端向量",
          "bge-m3",
          "gemini",
          "openai",
          "重算向量",
          "笔记检索",
        ],
        isAdvance: true,
        target: {
          settingsTab: "advance",
          advanceTab: "tools",
          toolGroup: "memory",
          openToolSubpage: true,
          anchor: "tools-subpage-panel",
        },
      },
      {
        id: "adv-tools-media-subpage",
        title: tr("ui.toolCalls.mediaTitle", "图片与视频识别"),
        description: tr("ui.toolCalls.mediaHint", ""),
        breadcrumbs: [navAdvance, tabTools, tr("ui.toolCalls.mediaTitle", "图片与视频识别")],
        aliases: ["readmediafile", "图片识别", "视频识别", "压缩边长", "jpeg质量", "视觉工具"],
        isAdvance: true,
        target: {
          settingsTab: "advance",
          advanceTab: "tools",
          toolGroup: "media",
          openToolSubpage: true,
          anchor: "tools-subpage-panel",
        },
      },
      {
        id: "adv-tools-reset-all",
        title: tr("ui.toolCalls.resetAll", "全部重置"),
        description: tr("ui.toolCalls.resetAllHint", ""),
        breadcrumbs: [navAdvance, tabTools],
        aliases: ["重置工具", "关闭全部工具", "reset tools"],
        isAdvance: true,
        target: {
          settingsTab: "advance",
          advanceTab: "tools",
          toolGroup: "general",
          openToolSubpage: true,
          anchor: "tools-reset-all",
        },
      },
    );

    // 动态工具包 + 工具包内各个独立工具
    for (const pack of toolStore.orderedPackages) {
      const packName = tr(`ui.toolCalls.nav.${pack.group}`, pack.group);
      const packDesc = tr(`ui.toolCalls.groups.${pack.group}`, "");
      items.push({
        id: `adv-tool-pack-${pack.group}`,
        title: packName,
        description: packDesc,
        breadcrumbs: [navAdvance, tabTools],
        rawKey: pack.group,
        aliases: [pack.group, packName, "工具包", "tool pack"],
        isAdvance: true,
        target: {
          settingsTab: "advance",
          advanceTab: "tools",
          toolGroup: pack.group,
          openToolSubpage: false,
        },
      });

      for (const toolName of pack.tools) {
        const displayName = toolDisplayName(toolName);
        const desc = toolDescription(toolName);
        items.push({
          id: `adv-tool-item-${pack.group}-${toolName}`,
          title: displayName,
          description: desc,
          breadcrumbs: [navAdvance, tabTools, packName],
          rawKey: toolName,
          aliases: [toolName, displayName, pack.group, packName],
          isAdvance: true,
          target: {
            settingsTab: "advance",
            advanceTab: "tools",
            toolGroup: pack.group,
            openToolSubpage: false,
            anchor: `tool-row-${toolName}`,
          },
        });
      }
    }

    // -----------------------------------------------------------------------
    // 6. 高级设置 -> 投屏设置 (Cast)
    // -----------------------------------------------------------------------
    items.push(
      {
        id: "adv-cast-enable",
        title: tr("settings.cast.enable", "投屏模式"),
        description: tr("settings.cast.enableDesc", ""),
        breadcrumbs: [navAdvance, tabCast],
        aliases: ["投屏", "串流", "cast", "esp32", "holocubic", "mjpeg"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "cast", anchor: "cast-enable" },
      },
      {
        id: "adv-cast-port",
        title: tr("settings.cast.port", "串流端口"),
        description: tr("settings.cast.portDesc", ""),
        breadcrumbs: [navAdvance, tabCast],
        aliases: ["端口", "port", "投屏端口"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "cast", anchor: "cast-port" },
      },
      {
        id: "adv-cast-performance",
        title: tr("settings.cast.performance", "性能设置"),
        description: `${tr("settings.cast.fps", "帧率")} · ${tr("settings.cast.quality", "画质")}`,
        breadcrumbs: [navAdvance, tabCast],
        aliases: ["帧率", "fps", "画质", "quality", "带宽"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "cast", anchor: "cast-performance" },
      },
      {
        id: "adv-cast-resolution",
        title: tr("settings.cast.resolution", "输出分辨率"),
        description: tr("settings.cast.resolutionDesc", ""),
        breadcrumbs: [navAdvance, tabCast],
        aliases: ["分辨率", "resolution", "320x240", "尺寸"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "cast", anchor: "cast-resolution" },
      },
      {
        id: "adv-cast-vivid",
        title: tr("settings.cast.vivid", "色彩增强 (vivid)"),
        description: tr("settings.cast.vividDesc", ""),
        breadcrumbs: [navAdvance, tabCast],
        aliases: ["vivid", "饱和度", "对比度", "小屏色彩"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "cast", anchor: "cast-vivid" },
      },
      {
        id: "adv-cast-dialog-hide",
        title: tr("settings.cast.dialogHide.title", "隐藏对话框"),
        description: tr("settings.cast.dialogHide.desc", ""),
        breadcrumbs: [navAdvance, tabCast],
        aliases: ["隐藏对话框", "纯立绘投屏"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "cast", anchor: "cast-dialog-hide" },
      },
      {
        id: "adv-cast-character-adjust",
        title: tr("settings.cast.characterAdjust", "人物调整"),
        description: tr("settings.cast.characterAdjustDesc", ""),
        breadcrumbs: [navAdvance, tabCast],
        aliases: ["投屏人物", "投屏缩放", "偏移"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "cast", anchor: "cast-character-adjust" },
      },
      {
        id: "adv-cast-dialog-adjust",
        title: tr("settings.cast.dialogAdjust", "对话框调整"),
        description: tr("settings.cast.dialogAdjustDesc", ""),
        breadcrumbs: [navAdvance, tabCast],
        aliases: ["投屏对话框", "字号", "背景不透明度"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "cast", anchor: "cast-dialog-adjust" },
      },
      {
        id: "adv-cast-control",
        title: tr("settings.cast.control", "控制"),
        description: `${tr("settings.cast.openWindow", "打开投屏窗口")} / ${tr("settings.cast.start", "开始串流")}`,
        breadcrumbs: [navAdvance, tabCast],
        aliases: ["投屏窗口", "开始串流", "停止串流"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "cast", anchor: "cast-control" },
      },
      {
        id: "adv-cast-connection",
        title: tr("settings.cast.connection", "连接地址"),
        description: tr("settings.cast.smallScreenHint", ""),
        breadcrumbs: [navAdvance, tabCast],
        aliases: ["mjpeg", "预览地址", "局域网ip", "流地址"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "cast", anchor: "cast-connection" },
      },
    );

    // -----------------------------------------------------------------------
    // 7. 高级设置 -> 永久记忆调试
    // -----------------------------------------------------------------------
    items.push(
      {
        id: "adv-memory-overview",
        title: tr("settings.memoryDebug.title", "永久记忆调试"),
        description: tr(
          "settings.memoryDebug.selectRoleHint",
          "查看角色记忆库内容与真实上下文，排查记忆压缩与注入问题",
        ),
        breadcrumbs: [navAdvance, tabMemory],
        aliases: ["memory debug", "记忆调试", "记忆库", "压缩指针", "真实上下文"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "memory", anchor: "memory-overview" },
      },
      {
        id: "adv-memory-compaction",
        title: tr("settings.memoryDebug.triggerTitle", "压缩状态（运行时生效值）"),
        description: tr("settings.memoryDebug.triggerHint", ""),
        breadcrumbs: [navAdvance, tabMemory],
        aliases: ["压缩进度", "触发间隔", "保留窗口", "memory trigger"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "memory", anchor: "memory-compaction" },
      },
      {
        id: "adv-memory-store",
        title: tr("settings.memoryDebug.storeTitle", "① 记忆库（存储真源）"),
        description: tr("settings.memoryDebug.storeHint", "查看短期摘要、长期事件、用户画像与约定"),
        breadcrumbs: [navAdvance, tabMemory],
        aliases: ["记忆真源", "长期事件", "用户画像", "promises"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "memory", anchor: "memory-store" },
      },
      {
        id: "adv-memory-injected",
        title: tr("settings.memoryDebug.injectedTitle", "② 注入视图"),
        description: tr("settings.memoryDebug.injectedHint", ""),
        breadcrumbs: [navAdvance, tabMemory],
        aliases: ["注入视图", "system memory", "实际注入"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "memory", anchor: "memory-injected" },
      },
      {
        id: "adv-memory-context",
        title: tr("settings.memoryDebug.contextTitle", "③ 真实上下文"),
        description: tr("settings.memoryDebug.contextEmpty", ""),
        breadcrumbs: [navAdvance, tabMemory],
        aliases: ["上下文", "messages", "prompt上下文"],
        isAdvance: true,
        target: { settingsTab: "advance", advanceTab: "memory", anchor: "memory-context" },
      },
    );

    // -----------------------------------------------------------------------
    // 8. 高级设置 -> 其他高级设置 (动态配置树 + 特殊附加区块)
    // -----------------------------------------------------------------------
    const configData = cachedEnvConfig.value;
    let ttsFirstSubcategory = "适配器 URL";

    if (configData) {
      for (const [catKey, catData] of Object.entries(configData)) {
        const catTitle = tr(`settings.advanceOther.categories.${catKey}`, catKey);
        const subEntries = Object.entries(catData.subcategories ?? {});
        if (catKey === "TTS 配置" && subEntries.length > 0) {
          ttsFirstSubcategory = subEntries[0][0];
        }

        for (const [subKey, subData] of subEntries) {
          const subTitle = tr(`settings.advanceOther.subcategories.${subKey}`, subKey);
          const subDesc = te(`settings.advanceOther.subcategoryDescs.${subKey}`)
            ? t(`settings.advanceOther.subcategoryDescs.${subKey}`)
            : t("settings.advanceOther.subcategoryDesc", { name: subTitle });

          items.push({
            id: `adv-other-sub-${catKey}-${subKey}`,
            title: subTitle,
            description: subDesc,
            breadcrumbs: [navAdvance, tabOther, catTitle],
            aliases: [catKey, subKey, catTitle, subTitle],
            isAdvance: true,
            target: {
              settingsTab: "advance",
              advanceTab: "other",
              category: catKey,
              subcategory: subKey,
            },
          });

          for (const setting of subData.settings ?? []) {
            const fieldLabel = tr(
              `settings.advanceOther.fields.${setting.key}`,
              setting.description || setting.key,
            );
            const valPreview = formatEnvValuePreview(setting.key, setting.value);
            const restartRequired = catKey !== "好感度" && setting.key !== "log.genai_debug";

            items.push({
              id: `adv-other-key-${catKey}-${subKey}-${setting.key}`,
              title: fieldLabel,
              description: valPreview ? `${subTitle} · ${valPreview}` : subDesc,
              breadcrumbs: [navAdvance, tabOther, catTitle, subTitle],
              rawKey: setting.key,
              restartRequired,
              aliases: [
                catKey,
                catTitle,
                subKey,
                subTitle,
                setting.key,
                setting.description || "",
                ...(ENV_KEY_ALIASES[setting.key] ?? []),
              ].filter(Boolean),
              isAdvance: true,
              target: {
                settingsTab: "advance",
                advanceTab: "other",
                category: catKey,
                subcategory: subKey,
                settingKey: setting.key,
              },
            });
          }
        }
      }
    }

    // 附加区块 1：TTS 连接控制
    items.push({
      id: "adv-other-tts-control",
      title: tr("settings.advanceOther.ttsControl.title", "TTS 连接控制"),
      description: tr("settings.advanceOther.ttsControl.desc", ""),
      breadcrumbs: [
        navAdvance,
        tabOther,
        tr("settings.advanceOther.categories.TTS 配置", "TTS 配置"),
      ],
      aliases: ["重连tts", "强制重连", "tts离线", "force reconnect", "tts control"],
      isAdvance: true,
      target: {
        settingsTab: "advance",
        advanceTab: "other",
        category: "TTS 配置",
        subcategory: ttsFirstSubcategory,
        anchor: "tts-control",
      },
    });

    // 附加区块 2：好感度动画
    items.push({
      id: "adv-other-affection-anim",
      title: tr("settings.advanceOther.affectionAnim.title", "好感度动画"),
      description: tr("settings.advanceOther.affectionAnim.desc", ""),
      breadcrumbs: [
        navAdvance,
        tabOther,
        tr("settings.advanceOther.categories.好感度", "好感度"),
        tr("settings.advanceOther.subcategories.通用", "通用"),
      ],
      aliases: ["心跳", "波浪", "心形", "好感度动画", "heartbeat", "wave", "即时生效"],
      isAdvance: true,
      target: {
        settingsTab: "advance",
        advanceTab: "other",
        category: "好感度",
        subcategory: "通用",
        anchor: "affection-anim",
      },
    });

    // -----------------------------------------------------------------------
    // 9. 顶栏其他一级设置页（跨页一键直达）
    // -----------------------------------------------------------------------
    const navText = tr("nav.text", "通用");
    const navBg = tr("nav.background", "背景");
    const navSound = tr("nav.sound", "声音");
    const navChar = tr("nav.character", "角色");
    const navAdv = tr("nav.adventure", "羁绊");
    const navHistory = tr("nav.history", "对话历史");
    const navAchievement = tr("nav.achievement", "成就");
    const navSave = tr("nav.save", "存档");
    const navLog = tr("nav.log", "日志");
    const navPlugins = tr("nav.plugins", "插件");

    // 9.1 通用 (SettingsText)
    items.push(
      {
        id: "tab-text-font",
        title: tr("settings.text.font.title", "界面字体"),
        description: tr("settings.text.font.selectHint", ""),
        breadcrumbs: [navText],
        aliases: ["字体", "font", "ttf", "woff2", "导入字体"],
        isAdvance: false,
        target: { settingsTab: "text", anchor: "text-font" },
      },
      {
        id: "tab-text-speed",
        title: tr("settings.text.speed.title", "文本显示速度"),
        description: tr("settings.text.sample.demo", ""),
        breadcrumbs: [navText],
        aliases: ["打字速度", "文字速度", "typing speed", "快慢"],
        isAdvance: false,
        target: { settingsTab: "text", anchor: "text-speed" },
      },
      {
        id: "tab-text-auto-advance",
        title: tr("settings.text.autoAdvance.title", "自动推进延迟"),
        description: tr(
          "advance.search.descs.textAutoAdvance",
          "调整开启自动对话后的语句切换停顿时间",
        ),
        breadcrumbs: [navText],
        aliases: ["自动推进", "auto advance", "自动播放延迟"],
        isAdvance: false,
        target: { settingsTab: "text", anchor: "text-auto-advance" },
      },
      {
        id: "tab-text-merge",
        title: tr("settings.text.mergeLine.title", "台词合并阈值"),
        description: `${tr("settings.text.mergeDelay.title", "合并延时")} · ${tr("settings.text.mergeMotion.title", "动作合并")}`,
        breadcrumbs: [navText],
        aliases: ["合并行", "台词合并", "动作描写", "merge line"],
        isAdvance: false,
        target: { settingsTab: "text", anchor: "text-merge-line" },
      },
      {
        id: "tab-text-reply-length",
        title: tr("settings.text.replyLength.title", "回复台词数量"),
        description: tr("settings.text.replyLength.desc", ""),
        breadcrumbs: [navText],
        aliases: ["回复长度", "台词数量", "简短回复", "reply length"],
        isAdvance: false,
        target: { settingsTab: "text", anchor: "text-reply-length" },
      },
      {
        id: "tab-text-sedentary",
        title: tr("settings.text.sedentary.title", "久坐与喝水提醒"),
        description: tr("settings.text.sedentary.desc", ""),
        breadcrumbs: [navText],
        aliases: ["久坐", "喝水", "健康提醒", "40分钟", "sedentary"],
        isAdvance: false,
        target: { settingsTab: "text", anchor: "text-sedentary" },
      },
      ...(import.meta.env.DEV
        ? [
            {
              id: "tab-text-devtools",
              title: tr("settings.text.vueDevTools.title", "Vue DevTools"),
              description: tr("settings.text.vueDevTools.desc", ""),
              breadcrumbs: [navText],
              aliases: ["devtools", "调试悬浮窗"],
              isAdvance: false,
              target: { settingsTab: "text", anchor: "text-devtools" },
            },
          ]
        : []),
      {
        id: "tab-text-memory",
        title: tr("settings.text.memory.title", "启用永久记忆"),
        description: tr("settings.text.memory.desc", ""),
        breadcrumbs: [navText],
        aliases: ["永久记忆开关", "长期记忆", "persistent memory"],
        restartRequired: true,
        isAdvance: false,
        target: { settingsTab: "text", anchor: "text-memory" },
      },
      {
        id: "tab-text-voice-sound",
        title: tr("settings.text.voiceSound.title", "对话音效"),
        description: tr("settings.text.voiceSound.desc", ""),
        breadcrumbs: [navSound],
        aliases: ["打字音", "对话音效", "voice sound", "嘟嘟声"],
        isAdvance: false,
        target: { settingsTab: "sound", anchor: "sound-voice-sound" },
      },
      {
        id: "tab-text-tts-cache",
        title: tr("settings.text.ttsCache.title", "语音缓存"),
        description: tr("advance.search.descs.textTtsCache", "检测与清理孤儿语音缓存文件"),
        breadcrumbs: [navText],
        aliases: ["语音缓存", "清理缓存", "孤儿文件", "tts cache"],
        isAdvance: false,
        target: { settingsTab: "text", anchor: "text-tts-cache" },
      },
      {
        id: "tab-text-codex-quota",
        title: tr("settings.text.codexQuota.title", "Codex 额度"),
        description: tr("settings.text.codexQuota.loginHint", ""),
        breadcrumbs: [navText],
        aliases: ["codex", "额度", "quota", "周额度", "5小时额度"],
        isAdvance: false,
        target: {
          settingsTab: "text",
          anchor: "text-codex-quota",
          fallbackAnchor: "text-update",
        },
      },
      {
        id: "tab-text-update",
        title: tr("settings.text.update.title", "版本更新"),
        description: `${tr("settings.text.update.checkButton", "检查更新")} · ${tr("settings.text.update.syncData", "同步数据")}`,
        breadcrumbs: [navText],
        aliases: ["检查更新", "版本号", "升级", "同步数据", "update"],
        isAdvance: false,
        target: { settingsTab: "text", anchor: "text-update" },
      },
      {
        id: "tab-text-lan-sync",
        title: tr("settings.text.lanSync.title", "局域网数据同步"),
        description: tr("settings.text.lanSync.desc", ""),
        breadcrumbs: [navText],
        aliases: ["局域网同步", "lan sync", "手机pc互传", "同步存档"],
        isAdvance: false,
        target: { settingsTab: "text", anchor: "text-lan-sync" },
      },
      {
        id: "tab-text-docs",
        title: tr("settings.text.docs.title", "查看文档了解 LingChat"),
        description: tr("settings.text.docs.desc", ""),
        breadcrumbs: [navText],
        aliases: ["文档", "docs", "帮助", "faq"],
        isAdvance: false,
        target: { settingsTab: "text", anchor: "text-docs" },
      },
      {
        id: "tab-text-back",
        title: `${tr("settings.text.back.refreshTts", "刷新 TTS 服务")} / ${tr("settings.text.back.clearHistory", "清除聊天历史")}`,
        description: tr("settings.text.back.title", "返回主菜单"),
        breadcrumbs: [navText],
        aliases: ["返回主菜单", "清除聊天历史", "刷新tts", "清空历史"],
        isAdvance: false,
        target: { settingsTab: "text", anchor: "text-back" },
      },
    );

    // 9.2 背景 (SettingsBackground)
    items.push(
      {
        id: "tab-bg-scene",
        title: tr("settings.background.scene.title", "场景管理"),
        description: `${tr("settings.background.scene.create", "创建场景")} · ${tr("settings.background.scene.upload", "上传背景")}`,
        breadcrumbs: [navBg],
        aliases: ["背景图片", "场景管理", "光影", "室内室外", "场景排序"],
        isAdvance: false,
        target: { settingsTab: "background", anchor: "bg-scene" },
      },
      {
        id: "tab-bg-particle",
        title: tr("settings.background.particle.title", "粒子选择"),
        description: [
          tr("settings.background.particle.starField", "星空"),
          tr("settings.background.particle.sakura", "樱花"),
          tr("settings.background.particle.fireworks", "烟花"),
          tr("settings.background.particle.ba", "星辉"),
          tr("settings.background.particle.fireflies", "萤火虫"),
          tr("settings.background.particle.meteorShower", "流星雨"),
        ].join(" · "),
        breadcrumbs: [navBg],
        aliases: ["粒子", "樱花", "星空", "烟花", "流星雨", "萤火虫", "粒子特效"],
        isAdvance: false,
        target: { settingsTab: "background", anchor: "bg-particle" },
      },
      {
        id: "tab-bg-weather",
        title: tr("settings.background.weather.title", "天气效果"),
        description: `${tr("settings.background.weather.city", "城市")} · ${tr("settings.background.weather.autoSync", "真实天气同步")}`,
        breadcrumbs: [navBg],
        aliases: ["天气", "下雨", "雷雨", "下雪", "暴雪", "起雾", "真实天气", "城市"],
        isAdvance: false,
        target: { settingsTab: "background", anchor: "bg-weather" },
      },
      {
        id: "tab-bg-anim-switch",
        title: tr("settings.background.animation.switchTitle", "动画开关"),
        description: `${tr("settings.background.animation.mouseTrail", "")} · ${tr("settings.background.animation.sceneAwareness", "")}`,
        breadcrumbs: [navBg],
        aliases: ["鼠标拖尾", "点击特效", "主菜单流星", "主菜单星空", "场景感知", "光标特效"],
        isAdvance: false,
        target: { settingsTab: "background", anchor: "bg-anim-switch" },
      },
      {
        id: "tab-bg-anim-settings",
        title: tr("settings.background.animation.settingsTitle", "动画参数"),
        description: `${tr("settings.background.animation.meteorFps", "流星帧率")} · ${tr("settings.background.animation.starsFps", "星空帧率")}`,
        breadcrumbs: [navBg],
        aliases: ["fps", "帧率", "流星帧率", "星空帧率"],
        isAdvance: false,
        target: { settingsTab: "background", anchor: "bg-anim-settings" },
      },
      ...(windows
        ? [
            {
              id: "tab-bg-hdr",
              title: tr("settings.background.hdr.title", "HDR 显示"),
              description: tr("settings.background.hdr.restartHint", ""),
              breadcrumbs: [navBg],
              aliases: ["hdr", "高动态范围"],
              restartRequired: true,
              isAdvance: false,
              target: { settingsTab: "background", anchor: "bg-hdr" },
            },
          ]
        : []),
      {
        id: "tab-bg-perf",
        title: tr("settings.background.perf.title", "性能检测"),
        description: tr("settings.background.perf.redetect", "重新检测"),
        breadcrumbs: [navBg],
        aliases: ["性能检测", "gpu", "cpu", "显卡检测", "推荐帧率"],
        isAdvance: false,
        target: { settingsTab: "background", anchor: "bg-perf" },
      },
    );

    // 9.3 声音 (SettingsSound)
    items.push(
      {
        id: "tab-sound-device",
        title: tr("settings.sound.device.title", "音频输出设备"),
        description: tr("settings.sound.device.selectTitle", ""),
        breadcrumbs: [navSound],
        aliases: ["扬声器", "耳机", "输出设备", "audio device"],
        isAdvance: false,
        target: { settingsTab: "sound", anchor: "sound-device" },
      },
      {
        id: "tab-sound-volumes",
        title: `${tr("settings.sound.volume.character", "人物音量")} / ${tr("settings.sound.volume.background", "背景音量")}`,
        description: [
          tr("settings.sound.volume.character", "人物音量"),
          tr("settings.sound.volume.bubble", "气泡音量"),
          tr("settings.sound.volume.background", "背景音量"),
          tr("settings.sound.volume.achievement", "成就音量"),
          tr("settings.sound.volume.ambient", "环境音音量"),
        ].join(" · "),
        breadcrumbs: [navSound],
        aliases: ["音量", "声音大小", "人物音量", "气泡音量", "成就音量", "环境音量", "volume"],
        isAdvance: false,
        target: { settingsTab: "sound", anchor: "sound-volume" },
      },
      {
        id: "tab-sound-test",
        title: tr("settings.sound.test.title", "声音测试"),
        description: `${tr("settings.sound.test.character", "")} / ${tr("settings.sound.test.bubble", "")} / ${tr("settings.sound.test.achievement", "")}`,
        breadcrumbs: [navSound],
        aliases: ["试音", "声音测试", "sound test"],
        isAdvance: false,
        target: { settingsTab: "sound", anchor: "sound-test" },
      },
      {
        id: "tab-sound-bgm",
        title: tr("settings.sound.bgm.title", "背景音乐设置"),
        description: tr("settings.sound.bgm.openFolder", ""),
        breadcrumbs: [navSound],
        aliases: ["bgm", "背景音乐", "歌单", "上传音乐", "随机播放"],
        isAdvance: false,
        target: { settingsTab: "sound", anchor: "sound-bgm" },
      },
      {
        id: "tab-sound-ambient",
        title: tr("settings.sound.ambient.title", "环境音管理"),
        description: tr("settings.sound.ambient.playing", ""),
        breadcrumbs: [navSound],
        aliases: ["环境音", "白噪音", "雨声", "ambient"],
        isAdvance: false,
        target: { settingsTab: "sound", anchor: "sound-ambient" },
      },
    );

    // 9.4 角色 / 羁绊 / 历史 / 成就 / 存档 / 日志 / 插件
    items.push(
      {
        id: "tab-char-list",
        title: tr("settings.character.list.title", "角色列表"),
        description: tr("advance.search.descs.charList", "切换当前角色、编辑角色配置或创建新角色"),
        breadcrumbs: [navChar],
        aliases: ["切换角色", "创建角色", "角色编辑", "立绘", "live2d", "桌宠", "热词"],
        isAdvance: false,
        target: { settingsTab: "character", anchor: "char-list" },
      },
      {
        id: "tab-char-identity",
        title: tr("settings.identity.title", "我的身份"),
        description: tr("settings.identity.hint", ""),
        breadcrumbs: [navChar],
        aliases: ["身份卡", "玩家身份", "称呼", "identity", "persona"],
        isAdvance: false,
        target: { settingsTab: "character", anchor: "char-identity" },
      },
      {
        id: "tab-char-import",
        title: tr("settings.character.import.title", "导入角色"),
        description: `${tr("settings.character.import.conflictPolicy", "冲突策略")} · zip / 7z`,
        breadcrumbs: [navChar],
        aliases: ["导入角色", "zip", "7z", "角色包"],
        isAdvance: false,
        target: { settingsTab: "character", anchor: "char-import" },
      },
      ...(!android
        ? [
            {
              id: "tab-char-folder",
              title: tr("settings.character.openFolder.title", "打开角色目录"),
              description: tr("settings.character.openFolder.button", ""),
              breadcrumbs: [navChar],
              aliases: ["角色文件夹", "character folder"],
              isAdvance: false,
              target: { settingsTab: "character", anchor: "char-open-folder" },
            },
          ]
        : []),
      {
        id: "tab-adv-bond",
        title: tr("settings.adventure.bond.title", "羁绊冒险"),
        description: `${tr("settings.adventure.standalone.title", "独立剧本")} · ${tr("settings.adventure.workshop.title", "创意工坊")}`,
        breadcrumbs: [navAdv],
        aliases: ["羁绊", "剧本", "独立剧本", "adventure", "剧情"],
        isAdvance: false,
        target: { settingsTab: "adventure", anchor: "adventure-bond" },
      },
      {
        id: "tab-history",
        title: tr("settings.history.title", "对话历史"),
        description: tr("settings.history.backtrackTip", ""),
        breadcrumbs: [navHistory],
        aliases: ["聊天记录", "回溯", "撤销对话", "history"],
        isAdvance: false,
        target: { settingsTab: "history", anchor: "history-top" },
      },
      {
        id: "tab-achievement",
        title: tr("settings.achievement.title", "成就"),
        description: tr("advance.search.descs.achievement", "查看已解锁与未解锁成就"),
        breadcrumbs: [navAchievement],
        aliases: ["成就", "achievement"],
        isAdvance: false,
        target: { settingsTab: "achievement", anchor: "achievement-top" },
      },
      {
        id: "tab-save",
        title: tr("settings.save.create.title", "新建存档与存档列表"),
        description: tr("settings.save.list.title", "存档列表"),
        breadcrumbs: [navSave],
        aliases: ["存档", "读档", "覆盖存档", "save", "load"],
        isAdvance: false,
        target: { settingsTab: "save", anchor: "save-create" },
      },
      {
        id: "tab-log",
        title: tr("settings.log.title", "日志"),
        description: `${tr("settings.log.autoScroll", "自动滚动")} · ${tr("settings.log.popout", "独立窗口")}`,
        breadcrumbs: [navLog],
        aliases: ["运行日志", "控制台", "独立日志窗口", "log"],
        isAdvance: false,
        target: { settingsTab: "log", anchor: "log-top" },
      },
      {
        id: "tab-plugins",
        title: tr("settings.plugins.title", "插件"),
        description: `${tr("settings.plugins.reload", "重新扫描插件")} · ${tr("settings.plugins.wsTitle", "WebSocket 连接")}`,
        breadcrumbs: [navPlugins],
        aliases: ["插件", "plugin", "扩展", "websocket", "manifest"],
        isAdvance: false,
        target: { settingsTab: "plugins", anchor: "plugins-top" },
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

      if (item.isAdvance) totalScore += 6;
      if (item.rawKey || item.target.settingKey || item.target.anchor?.includes("-")) {
        totalScore += 2;
      }

      scored.push({ item, score: totalScore });
    }

    scored.sort(
      (a, b) =>
        b.score - a.score ||
        Number(b.item.isAdvance) - Number(a.item.isAdvance) ||
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
