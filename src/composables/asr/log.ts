/** 诊断日志薄封装：统一 "[ASR]" 前缀，核心逻辑只调此封装（模块化审查）。
 *  传参 prefix 用于子模块（"[ASR/VAD]" / "[ASR/stream]"），输出与改造前完全一致。
 *  降频策略（feedVad/partial 计数）保持在调用点，本层不做节流。 */
export function asrLog(prefix = "[ASR]") {
  return {
    info: (...args: unknown[]) => console.log(prefix, ...args),
    warn: (...args: unknown[]) => console.warn(prefix, ...args),
    error: (...args: unknown[]) => console.error(prefix, ...args),
  };
}
