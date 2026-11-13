import { defineStore } from "pinia";
import {
  getToolCatalog,
  getToolRuntimeInfo,
  getToolSettings,
  saveToolSettings,
  TOOL_GROUP_KEYS,
  TOOL_PACKAGE_ORDER,
  type ToolCatalog,
  type ToolGroupCatalog,
  type ToolRuntimeInfo,
  type ToolSettings,
} from "@/api/services/tool-settings";
import { useUIStore } from "@/stores/modules/ui/ui";
import { i18n } from "@/locales";

const DEFAULT_MEDIA_PROMPT =
  "请详细识别并描述这个媒体文件的内容；如果其中包含文字、界面、人物、物体、动作或时间顺序，请准确说明。";

const DEFAULT_SETTINGS: ToolSettings = {
  web_search: {
    enabled: false,
    provider: "kimi",
    api_key: "",
    base_url: "",
    proxy_enabled: false,
    proxy_addr: "http://127.0.0.1:10808",
    max_results: 8,
    hide_search_results: false,
  },
  media_file: {
    image_enabled: true,
    video_enabled: true,
    max_file_mb: 100,
    image_max_edge: 2000,
    jpeg_quality: 85,
    max_output_tokens: 1024,
    default_prompt: DEFAULT_MEDIA_PROMPT,
  },
  embedding: {
    cloud_enabled: false,
    provider: "openai",
    base_url: "",
    api_key: "",
    model: "",
    timeout_secs: 30,
  },
  groups: {},
  enabled_tools: {},
  access_mode: "manual",
  max_tool_rounds: 8,
  tool_rag: true,
  tool_recommend_mode: "rag",
  tool_cache_capacity: 5,
};

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function clampInteger(value: unknown, fallback: number, min: number, max: number): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? Math.min(max, Math.max(min, Math.round(parsed))) : fallback;
}

/** 合并后端数据与默认值，避免旧配置缺字段导致模板访问 undefined。 */
function normalizeSettings(raw: ToolSettings): ToolSettings {
  const settings: ToolSettings = {
    ...clone(DEFAULT_SETTINGS),
    ...raw,
    web_search: { ...DEFAULT_SETTINGS.web_search, ...(raw.web_search ?? {}) },
    media_file: { ...DEFAULT_SETTINGS.media_file, ...(raw.media_file ?? {}) },
    embedding: { ...DEFAULT_SETTINGS.embedding, ...(raw.embedding ?? {}) },
    groups: { ...(raw.groups ?? {}) },
    enabled_tools: { ...(raw.enabled_tools ?? {}) },
  };
  // web_search 的开关以 WebSearchSettings.enabled 为准，白名单只是跟随它。
  settings.enabled_tools.web_search = settings.web_search.enabled ? ["web_search"] : [];

  settings.max_tool_rounds = clampInteger(settings.max_tool_rounds, 8, 1, 64);
  settings.tool_cache_capacity = clampInteger(settings.tool_cache_capacity, 5, 1, 20);
  settings.media_file.max_file_mb = clampInteger(settings.media_file.max_file_mb, 100, 1, 100);
  settings.media_file.image_max_edge = clampInteger(
    settings.media_file.image_max_edge,
    2000,
    512,
    4096,
  );
  settings.media_file.jpeg_quality = clampInteger(settings.media_file.jpeg_quality, 85, 50, 95);
  settings.media_file.max_output_tokens = clampInteger(
    settings.media_file.max_output_tokens,
    1024,
    128,
    4096,
  );
  if (!settings.media_file.default_prompt.trim()) {
    settings.media_file.default_prompt = DEFAULT_MEDIA_PROMPT;
  }
  settings.embedding.timeout_secs = clampInteger(settings.embedding.timeout_secs, 30, 5, 300);
  return settings;
}

/** 保存串行队列：快速连点时按顺序落盘，避免旧响应覆盖新状态。 */
let saveChain: Promise<void> = Promise.resolve();
let pendingSaves = 0;

export const useToolSettingsStore = defineStore("toolSettings", {
  state: () => ({
    settings: clone(DEFAULT_SETTINGS),
    catalog: { groups: [], residentTools: [], pluginTools: [] } as ToolCatalog,
    runtimeInfo: null as ToolRuntimeInfo | null,
    loading: false,
    saving: false,
    /** 子页草稿：高级设置 Tab 切换会卸载组件，草稿必须放在 store 里才不丢。 */
    drafts: {} as Record<string, ToolSettings>,
  }),

  getters: {
    /** 按设计稿的顺序排好、已合成的工具包；没有插件工具时不显示插件包。 */
    orderedPackages(state): ToolGroupCatalog[] {
      const packages: ToolGroupCatalog[] = [];
      for (const name of TOOL_PACKAGE_ORDER) {
        if (name === "plugin") {
          if (state.catalog.pluginTools.length > 0) {
            packages.push({
              group: "plugin",
              tools: [...state.catalog.pluginTools],
              supported: true,
            });
          }
          continue;
        }
        const group = state.catalog.groups.find((item) => item.group === name);
        if (group) packages.push(group);
      }
      return packages;
    },

    /** 当前勾选的工具总数（用于顶部统计）。 */
    totalEnabled(state): number {
      let count = 0;
      for (const group of state.catalog.groups) {
        if (group.group === "web_search") {
          if (state.settings.web_search.enabled) count += 1;
          continue;
        }
        const list = state.settings.enabled_tools[group.group] ?? [];
        count += group.tools.filter((tool) => list.includes(tool)).length;
      }
      const pluginList = state.settings.enabled_tools.plugin;
      count +=
        pluginList === undefined
          ? state.catalog.pluginTools.length
          : state.catalog.pluginTools.filter((tool) => pluginList.includes(tool)).length;
      return count;
    },
  },

  actions: {
    async load() {
      this.loading = true;
      try {
        const [settings, catalog, runtimeInfo] = await Promise.all([
          getToolSettings(),
          getToolCatalog(),
          getToolRuntimeInfo(),
        ]);
        this.settings = normalizeSettings(settings);
        this.catalog = catalog;
        this.runtimeInfo = runtimeInfo;
      } catch (error) {
        console.warn("加载工具配置失败:", error);
      } finally {
        this.loading = false;
      }
    },

    packageTools(group: string): string[] {
      if (group === "plugin") return this.catalog.pluginTools;
      return this.catalog.groups.find((item) => item.group === group)?.tools ?? [];
    },

    packageSupported(group: string): boolean {
      return this.catalog.groups.find((item) => item.group === group)?.supported ?? true;
    },

    /** 该工具包当前勾选的工具名。插件白名单缺省时视为全部勾选。 */
    checkedTools(group: string): string[] {
      if (group === "web_search") {
        return this.settings.web_search.enabled ? ["web_search"] : [];
      }
      if (group === "plugin") {
        return this.settings.enabled_tools.plugin ?? [...this.catalog.pluginTools];
      }
      return this.settings.enabled_tools[group] ?? [];
    },

    isToolChecked(group: string, tool: string): boolean {
      return this.checkedTools(group).includes(tool);
    },

    enabledCount(group: string): number {
      const checked = this.checkedTools(group);
      return this.packageTools(group).filter((tool) => checked.includes(tool)).length;
    },

    /** 把一份新的勾选清单写回 settings；web_search 与插件白名单分别处理。 */
    applyToolList(group: string, tools: string[]) {
      const list = [...new Set(tools)];
      if (group === "web_search") {
        this.settings.web_search.enabled = list.length > 0;
      }
      this.settings.enabled_tools[group] = list;
      if (group !== "web_search" && group !== "plugin") {
        this.settings.groups[group] = list.length > 0;
      }
      void this.saveNow();
    },

    setTool(group: string, tool: string, enabled: boolean) {
      const current = this.checkedTools(group);
      const next = enabled ? [...current, tool] : current.filter((name) => name !== tool);
      this.applyToolList(group, next);
    },

    setPackage(group: string, enabled: boolean) {
      this.applyToolList(group, enabled ? this.packageTools(group) : []);
    },

    resetPackage(group: string) {
      this.applyToolList(group, []);
    },

    /** 全部禁用 = 全局恢复默认（默认就是全部关闭）。 */
    disableAll() {
      for (const group of TOOL_GROUP_KEYS) {
        this.settings.groups[group] = false;
        this.settings.enabled_tools[group] = [];
      }
      this.settings.enabled_tools.web_search = [];
      this.settings.web_search.enabled = false;
      this.settings.enabled_tools.plugin = [];
      void this.saveNow();
    },

    resetAll() {
      this.disableAll();
    },

    /** Android 推荐配置：勾选除命令执行外的全部可用工具，一次保存。 */
    enableAndroidRecommended() {
      for (const group of this.orderedPackages) {
        if (!group.supported) continue;
        this.settings.enabled_tools[group.group] = [...group.tools];
        if (group.group !== "plugin" && group.group !== "web_search") {
          this.settings.groups[group.group] = group.tools.length > 0;
        }
      }
      this.settings.web_search.enabled = true;
      this.settings.enabled_tools.web_search = ["web_search"];
      void this.saveNow();
    },

    openDraft(group: string) {
      this.drafts[group] = clone(this.settings);
    },

    closeDraft(group: string) {
      delete this.drafts[group];
    },

    /** 提交子页草稿并保存。 */
    async saveDraft(group: string) {
      const draft = this.drafts[group];
      if (!draft) return;
      this.settings = normalizeSettings(clone(draft));
      // 归一化后的值回写草稿，避免测试连接等提前保存路径下界面与落盘值不一致。
      this.drafts[group] = clone(this.settings);
      await this.saveNow();
    },

    /** 串行保存；成功静默，失败用全局通知并回滚到后端状态。 */
    async saveNow() {
      const snapshot = clone(this.settings);
      if (snapshot.web_search.provider === "deepseek") {
        // DeepSeek 搜索走官方端点，残留的自定义地址会把请求打到错误地址。
        snapshot.web_search.base_url = "";
      }
      pendingSaves += 1;
      this.saving = true;
      saveChain = saveChain.then(async () => {
        try {
          await saveToolSettings(snapshot);
          this.runtimeInfo = await getToolRuntimeInfo().catch(() => this.runtimeInfo);
        } catch (error) {
          const ui = useUIStore();
          ui.showNotification({
            type: "error",
            title: i18n.global.t("ui.toolCalls.saveFailedTitle"),
            message: String(error),
            skipTipsCheck: true,
          });
          await this.load();
        } finally {
          pendingSaves -= 1;
          this.saving = pendingSaves > 0;
        }
      });
      await saveChain;
    },
  },
});
