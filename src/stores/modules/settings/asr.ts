import { defineStore } from "pinia";
import {
  asrGetSettings,
  asrListModels,
  asrListProviders,
  asrSetSettings,
  type AsrSettings,
  type ModelInfo,
  type ProviderInfo,
  type VadEvent,
} from "@/api/services/asr";

const DEFAULT_SETTINGS: AsrSettings = {
  active_provider: "qwen-asr",
  auto_listen: false,
  send_mode: "fill_only",
  stream_enabled: false,
  // 默认关闭：仅兜底全新用户（无 localStorage 记录时）；后端 load 结果与
  // persist 恢复值都会覆盖它
  voice_input_enabled: false,
  vad_silence_ms: 800,
  energy_warmup_ms: 100,
  // 逐帧 VAD 日志默认关（与后端 AsrSettings::defaults 一致）
  vad_debug_log: false,
  // 与后端 default_ptt_key 一致：默认裸 F8（ShortcutBinding JSON 格式）
  ptt_key: '{"key":"f8"}',
  // 与后端一致：全局快捷键默认关（OS 级抢占，不默认开启）
  ptt_global: false,
  provider_configs: {},
};

export const useAsrStore = defineStore("asr", {
  state: () => ({
    settings: { ...DEFAULT_SETTINGS } as AsrSettings,
    // 会话运行态（phase/activeSource）由 useAsrInput 模块级状态持有——
    // 它与录音采集私有变量强绑定，放 store 会分裂为两份状态。
    // store 只放跨组件 UI 需要的状态：micState / vadLoaded / lastError。
    lastError: null as string | null,
    vadEvent: null as VadEvent | null,
    providers: [] as ProviderInfo[],
    /**
     * provider id → 该 provider 的模型清单。
     *
     * 存**全部** provider 而非只存当前那个：设置页的服务选择是一个扁平列表，
     * 要一次把每个服务商的模型都列出来，不能只拉当前 provider。
     */
    modelsByProvider: {} as Record<string, ModelInfo[]>,
    micState: "idle" as "idle" | "recording" | "denied",
    vadLoaded: false,
  }),
  getters: {
    /**
     * 当前 provider 的模型清单。
     *
     * 保持**只读**语义（getter 而非 state）：`composables/asr/gates.ts` 的
     * `isStreamEnabled()` 读它做流式能力判定，改成 getter 后那边一行都不用动。
     * 写入走 {@link reloadModels}。
     */
    models: (state): ModelInfo[] => state.modelsByProvider[state.settings.active_provider] ?? [],
  },
  actions: {
    async load() {
      try {
        // 合并默认值：后端 settings.json 是权威（含 energy_warmup_ms 等全部
        // 设置字段，schema 已统一）；persist 恢复值只作未加载前的占位，
        // 后端数据 spread 在最后覆盖。localStorage 里旧的前端私有字段
        // 不参与决策（除被 excludePaths 剔除的 provider_configs）。
        this.settings = { ...DEFAULT_SETTINGS, ...this.settings, ...(await asrGetSettings()) };
        this.providers = await asrListProviders();
        // 逐个 provider 独立拉取并**独立 catch**：本地 llama-asr 的服务没启动
        // 是常态，不能让它一个失败就把整个扁平列表清空。
        await Promise.all(
          this.providers.map(async (p) => {
            this.modelsByProvider[p.id] = await asrListModels(p.id).catch(() => []);
          }),
        );
      } catch (e) {
        console.warn("[ASR] load failed:", e);
      }
    },
    /**
     * 重拉单个 provider 的模型清单（llama 换模型/重启后刷新用）。
     *
     * `region` 传调用方当前选中的地域（见 `asrListModels`）：设置页改地域后
     * 保存还没落盘时就重拉，不传会拿到旧地域的清单。
     *
     * 失败时清空该 provider 的条目**并抛出**——由调用方决定怎么提示
     * （设置页要显示"模型列表拉取失败"，静默吞掉会让用户以为服务端没有模型）。
     */
    async reloadModels(providerId: string, region?: string) {
      try {
        this.modelsByProvider[providerId] = await asrListModels(providerId, region);
      } catch (e) {
        this.modelsByProvider[providerId] = [];
        throw e;
      }
    },
    async save(s: AsrSettings) {
      try {
        await asrSetSettings(s);
        this.settings = s;
      } catch (e) {
        console.warn("[ASR] save failed:", e);
        throw e;
      }
    },
    onTurnCandidate(e: VadEvent) {
      this.vadEvent = e;
    },
    onTurnSealed(e: VadEvent) {
      this.vadEvent = e;
    },
    onSpeechStarted() {
      this.micState = "recording";
    },
    onError(code: string) {
      this.lastError = code;
    },
    /** 识别/连接成功路径清除错误（设置页状态面板据此转绿；失败只写不
     *  清会让"接上服务后仍红"——错误是运行时状态，见 persist exclude） */
    clearError() {
      this.lastError = null;
    },
    setMicState(s: "idle" | "recording" | "denied") {
      this.micState = s;
    },
    setVadLoaded(v: boolean) {
      this.vadLoaded = v;
    },
  },
  // api_key 唯一真相在后端 settings.json（tauri_plugin_store），
  // 不从 localStorage 持久化 provider_configs，避免明文 key 双副本。
  // 注意：exclude 只滤顶层 key，provider_configs 嵌在 settings 里，
  // 必须用 excludePaths 深度剔除（否则 api_key 明文落 localStorage）。
  // lastError 是运行时状态（上次会话/本次运行的服务错误），持久化会让
  // 设置页"接上服务后仍显示红色"（旧错误跨会话残留）。
  persist: {
    key: "lingchat-asr",
    // modelsByProvider 是每次 load 从后端拉取的瞬时数据（本地 llama 的清单甚至
    // 取决于服务是否在跑），持久化没有意义，还会让设置页在 load 完成前短暂
    // 显示上一次会话的陈旧模型列表。
    exclude: ["provider_configs", "lastError", "modelsByProvider"],
    excludePaths: ["settings.provider_configs"],
  },
});
