import { i18n } from "@/locales";
import type { ResourceKind, WsMode, WsState } from "@/api/services/plugins";

/**
 * 后端错误可能是纯错误码（`PLUGIN_MISSING_DEPENDENCY`），也可能是
 * 「错误码|补充信息」（`PLUGIN_INACTIVE_DEPENDENCY|base_lib`）。
 * 查表翻译错误码，补充信息附在下一行；查不到则原样显示（兼容其它字符串错误）。
 */
export const formatPluginError = (raw: string): string => {
  const [code, ...rest] = raw.split("|");
  const detail = rest.join("|").trim();
  const base = i18n.global.t(`settings.plugins.errors.${code}`, code);
  return detail ? `${base}\n${detail}` : base;
};

const WS_STATE_DOT: Record<WsState, string> = {
  connected: "bg-emerald-400",
  connecting: "bg-amber-400 animate-pulse",
  error: "bg-red-400",
  stopped: "bg-white/30",
};

/** WS 连接状态 → 小圆点的颜色类。 */
export const wsStateDot = (state: WsState): string => WS_STATE_DOT[state] ?? "bg-white/30";

/** WS 连接状态 → 展示文案。 */
export const wsStateLabel = (state: WsState): string =>
  i18n.global.t(`settings.plugins.wsState.${state}`);

/** WS 连接方向 → 展示文案。 */
export const wsModeLabel = (mode: WsMode): string =>
  i18n.global.t(`settings.plugins.wsMode.${mode}`);

/** 插件携带的资源类型 → 展示文案。 */
export const kindLabel = (kind: ResourceKind): string =>
  i18n.global.t(`settings.plugins.resourceKinds.${kind}`);
