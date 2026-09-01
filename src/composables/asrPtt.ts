/**
 * PTT（按住说话）手势状态机 —— 纯事件驱动、无副作用、可单测。
 *
 * 三手势：长按（≥250ms 松开 = 说一段结束识别）、单击（<250ms 松开 = toggle
 * 保持录音，再按一次结束）、再次按下（toggle-held 中且会话已被外部丢弃 = 一次
 * 按键重启录音）。与录音链路完全解耦：本模块只做状态转移与手势判定，返回
 * 指令（PttCommand），动作（start/stop/discard）由 useAsrInput 执行。
 *
 * 设计动机（学科审查 P1）：状态机转移矩阵是最值得单测的逻辑——此前埋在
 * useAsrInput 的 pttDown/pttUp 里不可测，抽离为纯函数后可用 fake timers
 * 穷举验证。本项目前端暂无测试基建，结构已就位，测试框架引入时本模块
 * 无任何 DOM/Vue 依赖，可直接测。
 *
 * 状态语义：
 * - none：未按
 * - pending：按下未超阈值（等待 keyup 判定单击/长按）
 * - held：长按中（keyup 时结束识别）
 * - toggle-held：单击保持录音中（再按 keydown 结束）
 */
export type PttState = "none" | "pending" | "held" | "toggle-held";

/** 单击/长按判定阈值（毫秒）：keyup 在阈值内松开 = 单击（toggle 保持录音），
 *  超过 = 长按（结束识别） */
export const PTT_TAP_THRESHOLD_MS = 250;

/** 状态机指令：useAsrInput 据此执行副作用（录音启动/结束/重启） */
export type PttCommand =
  | { kind: "start" }
  | { kind: "stop" } /** toggle-held 中按下但会话已被外部 discard：一次按键重启录音 */
  | { kind: "restart" }
  | { kind: "none" };

let state: PttState = "none";
let tapTimer: number | undefined;

function clearTapTimer(): void {
  if (tapTimer !== undefined) {
    window.clearTimeout(tapTimer);
    tapTimer = undefined;
  }
}

/** 当前手势状态（只读判定用） */
export function getPttState(): PttState {
  return state;
}

/** 是否处于任一活跃手势（非 none）——handleWindowBlur 判断"按键在按着" */
export function isPttActive(): boolean {
  return state !== "none";
}

/** 复位：清计时器 + 回 none。会话 teardown/blur/门控拒绝路径统一调用，
 *  保证任何中断都不残留 pending/held/toggle-held */
export function resetPtt(): void {
  clearTapTimer();
  state = "none";
}

/**
 * keydown 推进。
 * @param sessionActive 当前会话是否为"本按键启动的录音"（recording 且 button 源）
 * @param sessionBusy   是否有任何会话在飞（mic/auto 录音中，PTT 不打断）
 */
export function pttKeyDown(sessionActive: boolean, sessionBusy: boolean): PttCommand {
  // toggle-held（单击保持录音）中再次按下：结束；会话已被外部 discard 则重启
  if (state === "toggle-held") {
    state = "none";
    return sessionActive ? { kind: "stop" } : { kind: "restart" };
  }
  // 按住中（pending/held）或其它会话在飞：不打断
  if (state !== "none" || sessionBusy) return { kind: "none" };
  // 开始 pending：阈值内松开由 keyup 判定单击，否则转 held
  state = "pending";
  tapTimer = window.setTimeout(() => {
    if (state === "pending") state = "held";
  }, PTT_TAP_THRESHOLD_MS);
  return { kind: "start" };
}

/**
 * keyup 推进。
 * @param sessionActive 当前会话是否为"本按键启动的录音"（recording 且 button 源）
 */
export function pttKeyUp(sessionActive: boolean): PttCommand {
  if (state === "pending") {
    // 单击：松开不结束，录音保持（toggle-held），下次按下再结束；
    // 会话已被外部 discard（sessionActive=false）：复位不残留
    clearTapTimer();
    state = sessionActive ? "toggle-held" : "none";
    return { kind: "none" };
  }
  if (state === "held") {
    // 长按结束：停止 → 识别 → 按 send_mode 处理
    state = "none";
    return sessionActive ? { kind: "stop" } : { kind: "none" };
  }
  if (state === "toggle-held") {
    // 正常保持录音中 keyup 无操作；被外部 discard 后复位
    if (!sessionActive) state = "none";
    return { kind: "none" };
  }
  return { kind: "none" };
}
