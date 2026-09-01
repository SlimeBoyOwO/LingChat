import { asrLog } from "./log";
import { canStartAsr, chatActive } from "./gates";
import { discardRecording, start, stop, toggleAutoListenFunction } from "./session";
import { getPttState, pttKeyDown, pttKeyUp, resetPtt } from "./ptt-state";
import { activeSource, phase, runtime } from "./state";
import { bindingMatches, isDirKey, parsePttBinding, type ShortcutBinding } from "@/utils/shortcuts";

/** 解析持久化的快捷键绑定；非法 JSON / 缺字段回退默认裸键。
 *  每次按键实时解析（毫秒级），改动设置立即生效，无需缓存失效机制。
 *  解析策略统一走 utils/shortcuts 的 parsePttBinding（设置页显示同款）。 */
function resolvePttBinding(): ShortcutBinding {
  return parsePttBinding(runtime.asrStore?.settings.ptt_key) ?? { key: "f8" };
}

// ── 按住说话(PTT)快捷键 v2：auto_listen 模式关 = 长按 PTT / 单击 toggle 双语义；
// 模式开 = 快捷键专职切换自动监听启用开关（只动运行态，不碰模式设置）。
// 窗口内生效（/chat 与 /pet，路由门控在 chatActive）；ptt_global 开启时退位给全局
// 快捷键事件（任意应用前台可用，blur 不丢录音——释放事件由全局回调保证到达）。
// 复用 button 手动语义（fill_only 拼接、跳过显示锁；总开关一律生效）。
// 手势状态机判定在 ptt-state（纯逻辑可单测），此处只做门控与动作执行。

/** PTT 按下统一入口（窗口内 keydown 与全局快捷键共用）：
 *  门控（总开关/路由/AI 生成/TTS 播放等）+ 状态机推进。 */
export function pttDown() {
  // 总开关门控（用户需求：两种行为都只能在语音输入总开关开启后生效）
  if (!runtime.asrStore?.settings.voice_input_enabled) return;
  // 行为 2：auto_listen 模式开 → 快捷键专职切换启用开关（toggleAutoListenFunction 只动运行态）。
  // 与行为 1 一致：仅聊天上下文（/chat 与 /pet，抽屉未开）生效——主菜单等路由
  // 翻转 autoListenActive 会在进聊天后未经交互启动自动监听
  if (runtime.asrStore?.settings.auto_listen) {
    if (!chatActive.value) return;
    toggleAutoListenFunction();
    return;
  }
  // 行为 1：模式关 → 长按 PTT / 单击 toggle 双语义
  const sessionActive = phase.value === "recording" && activeSource.value === "button";
  const sessionBusy = phase.value !== "idle";
  const cmd = pttKeyDown(sessionActive, sessionBusy);
  if (cmd.kind === "stop") {
    // toggle-held（单击保持录音）中按下即结束
    stop();
    return;
  }
  if (cmd.kind === "start" || cmd.kind === "restart") {
    // 门控（手动语义）：路由非 /chat+/pet、抽屉开、AI 生成中、TTS 播放中等 → 静默拒绝；
    // restart = toggle-held 中按下但会话已被外部 discard（AI 进 thinking 等）→ 一次按键重启
    if (!chatActive.value || !canStartAsr(false, true)) {
      // 门控拒绝：状态机复位（不残留 pending），后续 keyup 落空无害
      resetPtt();
      return;
    }
    // 立即开始录音（不丢首字）；250ms 内松开由 keyup 判定为单击，否则 keyup 时按长按结束
    void start("button").catch((err) => {
      // start 门控拒绝是静默 return 不 throw；只有 getUserMedia 等失败才走这里
      resetPtt();
      asrLog().warn("PTT start failed:", err);
    });
  }
}

/** PTT 松开统一入口（窗口内 keyup 与全局快捷键共用）：手势判定在 ptt-state，
 *  这里只执行动作（长按结束 = 停止 → 识别 → 按 send_mode 处理）。 */
export function pttUp() {
  const sessionActive = phase.value === "recording" && activeSource.value === "button";
  const cmd = pttKeyUp(sessionActive);
  if (cmd.kind === "stop") {
    stop();
  }
}

export function handlePttKeyDown(e: KeyboardEvent) {
  // 全局模式开启且实际已注册：窗口内监听退位，由全局事件驱动（单一输入源，
  // 防双触发——OS 全局快捷键在应用聚焦时也触发，双源必打架）。
  // 注册失败（pttGlobalOk=false，键被占用/启动 sync 失败）时不退位：全局没生效，
  // 窗口内监听兜底（审查中危 1：防"设置开但全局没注册 + 窗口内退位"双重失效）
  if (runtime.asrStore?.settings.ptt_global && runtime.pttGlobalOk) return;
  if (!bindingMatches(resolvePttBinding(), e) || e.repeat) return;
  pttDown();
}

export function handlePttKeyUp(e: KeyboardEvent) {
  // 注意：keyup 不做全局模式退位（审查 M-1）——keydown 早退防的是双触发
  // （双 toggle），keyup 无此必要：状态机对重复 keyup 幂等（第二次落 none
  // 无副作用）。而 pttGlobalOk 的 false→true 翻转可以发生在一次按键的
  // down 与 up 之间（注册失败时窗口内监听 → 期间一次成功保存 emit ok:true），
  // 若 keyup 也早退会吞掉松开事件 → 状态机滞留 held、录音只能等 60s 硬上限
  // 并整段识别发送。窗口内 keyup 与全局 released 双到达时同样安全：
  // 第一次 held→stop，第二次 none 无动作。
  // 主键比对（松开瞬间修饰键状态不可靠）；大小写不敏感——KeyboardEvent.key
  // 对功能键返回大写，且 binding.key 可能被手改为大写（v1 硬编码 PTT_KEY='F8'
  // 大写），故双侧归一——与 bindingMatches 的归一约定一致，避免 keydown 匹配
  // 而 keyup 不匹配导致录音挂起。方向键成对匹配（与 keydown 的 isDirKey 语义
  // 一致，避免绑定方向键后 keydown 配对触发、keyup 只比主键导致录音无法结束）
  const binding = resolvePttBinding();
  const k = e.key.toLowerCase();
  const bk = binding.key.toLowerCase();
  const isDir = isDirKey(bk);
  if (isDir ? k !== "arrowup" && k !== "arrowdown" : k !== bk) return;
  pttUp();
}

export function handleWindowBlur() {
  // 全局模式注册成功：失焦是常态（这正是功能场景），释放事件由全局快捷键回调
  // 保证到达（GetAsyncKeyState 轮询与焦点无关），不能丢弃录音。
  // 注册失败（pttGlobalOk=false）时无全局回调 → 与窗口内模式一致走 blur 兜底
  if (runtime.asrStore?.settings.ptt_global && runtime.pttGlobalOk) return;
  const pttState = getPttState();
  if (pttState === "none") return;
  // 按住中（pending/held）失焦：keyup 收不到，丢弃录音（不识别，避免把环境声
  // 送去识别），防录音卡死——blur 兜底语义只对"按键物理按下中"成立
  const held = pttState === "pending" || pttState === "held";
  resetPtt();
  if (phase.value === "recording" && activeSource.value === "button") {
    if (!held) {
      // toggle-held（单击保持）：按键早已松开，不存在丢失的 keyup——仅复位
      // 按键态，保留录音（审查 M-2：此前任意失焦即丢弃，tap-to-toggle 是
      // 最常见手势，点窗口内其它区域就静默丢话）。失焦期间按键停止不可达，
      // 但有 60s 硬上限 + 回焦后 PTT 可停 + mic 按钮可停三条逃生路径
      asrLog().info("PTT toggle-held 窗口失焦，保留录音");
      return;
    }
    asrLog().info("PTT 窗口失焦，丢弃录音");
    discardRecording();
  }
}
