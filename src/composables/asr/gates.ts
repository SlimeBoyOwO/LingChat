import { computed } from "vue";

import { asrLog } from "./log";
import { asrLockedUntil, runtime, voicePlaying } from "./state";

// /chat（主界面）与 /pet（桌宠）都算聊天场景；设置抽屉打开时不可用
export const chatActive = computed(() => {
  if (!runtime.route || !runtime.uiStore) return false;
  return (
    (runtime.route.path === "/chat" || runtime.route.path === "/pet") &&
    !runtime.uiStore.showSettings
  );
});

// ── ASR 可用性门控（§1 全 12 项） ──────────────────────────────
// 综合判定当前能否启动 ASR 录音（所有禁用条件取 OR）。编号表仅罗列全部
// 条件，与代码执行顺序无关：
// 1-3. currentStatus ∈ {thinking, responding, presenting}
// 4.    command === 'touch'（触摸模式）
// 5.    showMobileMenu === true（移动端菜单展开）
// 6.    route.path !== '/chat'
// 7.    uiStore.showSettings === true
// 8.    runningScript && choices.length > 0（剧本选择分支）
// 9.    loadingComplete === false（启动动画未完成）
// 10.   显示锁未过期（识别结果填入后短暂禁止再触发；ignoreLock 供监测启停跳过）
// 11.   语音输入总开关关（自动与手动录音都被挡——总开关是整体语音输入开关）
// 12.   TTS 播放中（外放语音会被误识别）
// 任何一项满足即视为不可用。start() / startEnergyMonitor RMS 触发 / 按钮 enable 都查它。
// forManual=true（手动 mic 录音）：仅跳过 10 显示锁——锁防的是 auto 触发覆盖识别
// 结果（手动是用户主动，不受锁限）；总开关一律生效。ignoreLock=true（监测启停用）：
// 锁只挡"触发录音"，不挡"监测启停"——否则识别完成后锁一设监测就停、锁过期无人复活。
export function canStartAsr(opts: { ignoreLock?: boolean; forManual?: boolean } = {}): boolean {
  const { ignoreLock = false, forManual = false } = opts;
  if (!runtime.route || !runtime.uiStore || !runtime.gameStore) return false;
  // 6 + 7：路由/抽屉门控（chatActive 已是这两项的合成；/chat 与 /pet 均可）
  if (
    (runtime.route.path !== "/chat" && runtime.route.path !== "/pet") ||
    runtime.uiStore.showSettings
  )
    return false;
  // 9：LoadingTransition 启动动画未完成（§1.9）
  if (!runtime.gameStore.loadingComplete) return false;
  // 1-3：核心对话状态
  if (runtime.gameStore.currentStatus !== "input") return false;
  // 4：触摸模式
  if (runtime.gameStore.command === "touch") return false;
  // 5：移动端菜单展开
  if (runtime.mobileMenuOpen) return false;
  // 8：剧本选择分支
  const script = (runtime.gameStore as unknown as { runningScript?: { choices?: unknown[] } })
    .runningScript;
  if (script && Array.isArray(script.choices) && script.choices.length > 0) return false;
  // 11：语音输入总开关——整体语音输入开关（自动与手动都被挡）
  if (!runtime.asrStore?.settings.voice_input_enabled) return false;
  // 12：角色语音（TTS）播放中（外放 TTS 进麦克风 → 误识别 AI 自己的话）
  if (voicePlaying.value) return false;
  // 10：识别结果短暂显示锁（fill_only 填入 inputMessage 到自动 send 的窗口期）。
  // ignoreLock=true 供 updateAsrAvailability 用：锁只挡"触发录音"，不挡"监测启停"——
  // 否则识别完成后锁一设监测就停、锁过期无人复活（触发后死锁）。
  // forManual=true（手动 mic）跳过锁：显示锁防的是 auto RMS 自动触发覆盖识别结果，
  // 手动点击是用户主动（fill_only 持续录入），不受锁限。
  if (!ignoreLock && !forManual && Date.now() < asrLockedUntil.value) return false;
  return true;
}

/** 流式判定缓存（降频诊断）：isStreamEnabled 在录音数据路径每个音频块都会
 *  被调用（30+/s），固定打印会刷屏——只在判定结果变化时输出 */
let lastStreamEnabled: boolean | null = null;

/** 流式是否生效：设置开关 + 当前生效模型的流式能力（模型级权威判定，
 *  元数据全部来自后端 asr_list_models——前端不再维护硬编码集合） */
export function isStreamEnabled(): boolean {
  if (!runtime.asrStore?.settings.stream_enabled) {
    // early-return 也同步缓存——否则关→开的切换不打日志
    // （cache 残留 true，重开后判定相等被跳过），排查"partial 为何不来"会误判
    lastStreamEnabled = false;
    return false;
  }
  const sel =
    runtime.asrStore.settings.provider_configs[runtime.asrStore.settings.active_provider]?.model ??
    "";
  const model =
    runtime.asrStore.models.find((m) => m.id === sel) ??
    runtime.asrStore.models.find((m) => m.is_default);
  // 模型清单未加载（拉取失败等）时流式判定为 false → 走整句识别；
  // 配置了流式模型却降级整句的代价是"无 partial"，后端能力不受影响
  const enabled = model?.supports_streaming ?? false;
  // 诊断（审查降频）：判定变化时暴露依据（模型清单是否命中、命中哪个模型）
  if (enabled !== lastStreamEnabled) {
    lastStreamEnabled = enabled;
    asrLog().info(
      `isStreamEnabled: stream=${runtime.asrStore.settings.stream_enabled}, ` +
        `model=${sel || "(default)"}${model ? ` (${model.supports_streaming ? "stream" : "batch"})` : " (未加载)"} → ${enabled}`
    );
  }
  return enabled;
}

/** SSE 类结果流式 provider 判定（llama-asr 起步；名字带 llama 是历史遗留）。
 *  语义：整段 WAV 上传 + SSE 增量 partial（stop 后到达），与 qwen WS 真流式
 *  （边录边发 PCM）分流。
 *
 *  ⚠️ 扩展点：新增 SSE 类 provider（如未来的 openai-compatible 泛化入口，
 *  见 provider.rs 扩展指南）必须同步加进此集合——llama-asr 与它同链路
 *  （不建 WS 会话、stop 时整段上传、partial 在 recognizing 阶段放行）。 */
export function isLlamaStream(): boolean {
  const sseProviders = ["llama-asr"];
  return (
    sseProviders.includes(runtime.asrStore?.settings.active_provider ?? "") && isStreamEnabled()
  );
}
