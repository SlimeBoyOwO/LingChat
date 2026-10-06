/**
 * 音频频谱的样式与配色
 *
 * SpectrumVisualizer（绘制）与 SoundEffectPanel（配置）共用，
 * 新增配色/样式只需在这里加一项，两处自动生效。
 */

// ===== 配色 =====
export interface SpectrumPalette {
  id: string;
  /** 展示名（配色名保留原文，不参与 i18n） */
  label: string;
  from: string;
  to: string;
}

export const SPECTRUM_PALETTES: SpectrumPalette[] = [
  { id: "aurora", label: "极光 Aurora", from: "#79d9ff", to: "#a78bfa" },
  { id: "ocean", label: "深海 Ocean", from: "#22d3ee", to: "#3b82f6" },
  { id: "sakura", label: "樱 Sakura", from: "#ffb3d1", to: "#ff6f9c" },
  { id: "ember", label: "余烬 Ember", from: "#fbbf24", to: "#fb7185" },
  { id: "mint", label: "薄荷 Mint", from: "#5eead4", to: "#4ade80" },
  { id: "galaxy", label: "星河 Galaxy", from: "#818cf8", to: "#e879f9" },
];

/** 默认配色（与界面主色 #79d9ff 同源） */
export const DEFAULT_SPECTRUM_PALETTE = "aurora";

/** 自定义配色的 id（选中时取 audio.spectrumColor1 / spectrumColor2） */
export const CUSTOM_SPECTRUM_PALETTE = "custom";

/** 自定义配色初始值 */
export const DEFAULT_SPECTRUM_COLOR_FROM = "#79d9ff";
export const DEFAULT_SPECTRUM_COLOR_TO = "#a78bfa";

/** 取配色实际颜色（预设 → from/to；custom 或未知 id → 自定义颜色兜底） */
export function resolveSpectrumColors(
  paletteId: string | undefined,
  customFrom?: string,
  customTo?: string,
): { from: string; to: string } {
  const preset = SPECTRUM_PALETTES.find((p) => p.id === paletteId);
  if (preset) return { from: preset.from, to: preset.to };
  return {
    from: customFrom || DEFAULT_SPECTRUM_COLOR_FROM,
    to: customTo || DEFAULT_SPECTRUM_COLOR_TO,
  };
}

// ===== 样式 =====
/**
 * 频谱形态：
 * - mirror 镜像：以中线为轴的胶囊，最省高度，迷你尺寸下最耐看
 * - bars   柱状：从底部生长的经典柱状
 * - ring   圆环：绕圆周辐射的环形频谱
 */
export type SpectrumStyle = "mirror" | "bars" | "ring";

export const SPECTRUM_STYLES: SpectrumStyle[] = ["mirror", "bars", "ring"];

export const DEFAULT_SPECTRUM_STYLE: SpectrumStyle = "mirror";

/** 把持久化里的任意值收敛成合法样式（旧数据/手改设置都不至于画不出来） */
export function resolveSpectrumStyle(value: string | undefined): SpectrumStyle {
  return (SPECTRUM_STYLES as string[]).includes(value ?? "")
    ? (value as SpectrumStyle)
    : DEFAULT_SPECTRUM_STYLE;
}
