/**
 * 光影 → CSS 的纯函数换算。
 *
 * 这里只负责「给一组参数，产出各层 style」，不读任何 store；生效参数的解析与
 * 开关屏蔽在 `stores/modules/lighting` 里做完再传进来。分开是因为渲染层分散在
 * 三个组件（背景 / 舞台 / 立绘），而参数换算只该有一份真相。
 */
import type { CSSProperties } from "vue";
import type { FilterParams, LightingParams } from "@/api/services/scene";

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

/** 一个光影层：样式 + 可选动画类。 */
export interface LightingLayer {
  style: CSSProperties;
  className?: string;
}

/**
 * 呼吸动画靠 `opacity` 的 keyframes 实现（只走合成器，不触发重绘）。keyframes 读
 * `--lighting-base`，所以本征不透明度必须由这里一并给出，否则动画会把图层自身的
 * opacity 直接覆盖掉。
 */
function breathe(layer: CSSProperties, l: LightingParams, baseOpacity: number): LightingLayer {
  if (!l.breathing_enabled) return { style: layer };
  const amount = clamp01(l.breathing_amount ?? 0.18);
  return {
    className: "lighting-breath",
    style: {
      ...layer,
      "--lighting-base": String(baseOpacity),
      "--lighting-breath-period": `${Math.max(1, l.breathing_period ?? 7)}s`,
      "--lighting-breath-lo": String(1 - amount),
      "--lighting-breath-hi": String(1 + amount),
    } as CSSProperties,
  };
}

/**
 * 滤镜串。轮廓光（rim light）用带偏移的 `drop-shadow` 实现——它按图片自身的
 * alpha 剪影描边，因此不需要任何遮罩就能沿人物轮廓发光。
 */
function buildFilter(f: FilterParams | undefined): string | undefined {
  if (!f) return undefined;
  const parts: string[] = [];
  if (f.brightness !== 1) parts.push(`brightness(${f.brightness})`);
  if (f.contrast !== 1) parts.push(`contrast(${f.contrast})`);
  if (f.saturation !== 1) parts.push(`saturate(${f.saturation})`);
  if (f.glow_radius > 0) parts.push(`drop-shadow(0 0 ${f.glow_radius}px ${f.glow_color})`);
  if (f.sepia > 0) parts.push(`sepia(${f.sepia})`);
  if (f.rim_enabled && (f.rim_blur ?? 0) > 0) {
    parts.push(
      `drop-shadow(${f.rim_dx ?? 0}px ${f.rim_dy ?? 0}px ${f.rim_blur}px ${f.rim_color ?? "#ffffff"})`
    );
  }
  return parts.length > 0 ? parts.join(" ") : undefined;
}

/** 旧版的径向光照层：`overlay_target` 决定它压在背景上、角色上还是两者。 */
function buildOverlay(
  l: LightingParams,
  want: "background" | "character"
): CSSProperties | undefined {
  if (!l.overlay_enabled) return undefined;
  if (l.overlay_target !== "both" && l.overlay_target !== want) return undefined;
  const blend = l.blend_mode !== "normal" ? l.blend_mode : "overlay";
  return {
    background: `radial-gradient(circle at ${l.light_x}% ${l.light_y}%, ${l.overlay_color1} 0%, ${l.overlay_color2} ${l.overlay_radius}%)`,
    mixBlendMode: blend as CSSProperties["mixBlendMode"],
    opacity: String(l.overlay_opacity),
  };
}

/**
 * 方向光：两层线性渐变，受光侧 `screen` 提亮、背光侧 `multiply` 压暗。
 *
 * `light_angle` 沿用 CSS 的罗盘约定（0deg = 光源在正上方，顺时针增大）。
 * `linear-gradient(θdeg, A, B)` 把 A 放在「背离光源」那条边、B 放在「朝向光源」
 * 那条边，所以暖色必须是最后一个色标。
 */
function buildDirectional(l: LightingParams, breathing: boolean) {
  const angle = l.light_angle ?? 315;
  const softness = clamp01(l.light_softness ?? 0.55);
  const strength = clamp01(l.light_strength ?? 0.5);
  // 过渡带宽度：柔和度 0 → 明暗在中线硬切；1 → 渐变铺满几乎整个画面
  const spread = 25 + softness * 65;
  const lit = {
    background: `linear-gradient(${angle}deg, transparent ${100 - spread}%, ${
      l.light_warm_color ?? "#ffd9a0"
    } 100%)`,
    mixBlendMode: "screen" as const,
    opacity: String(strength),
  };
  const shadow = {
    background: `linear-gradient(${angle}deg, ${
      l.shadow_cool_color ?? "#16233b"
    } 0%, transparent ${spread}%)`,
    mixBlendMode: "multiply" as const,
    opacity: String(strength),
  };
  const withBreath = (layer: CSSProperties, base: number): LightingLayer =>
    breathing ? breathe(layer, l, base) : { style: layer };
  return {
    lit: withBreath(lit, strength),
    shadow: withBreath(shadow, strength),
  };
}

/**
 * 冷暖分离：`mix-blend-mode: color` 只借用本层的色相与饱和度、亮度沿用画面，
 * 所以染色不会改变明暗关系——换成 overlay / soft-light 会把暗角越打越黑。
 */
function buildGrade(l: LightingParams, breathing: boolean): LightingLayer {
  const angle = l.light_angle ?? 315;
  const strength = clamp01(l.grade_strength ?? 0.35);
  const style: CSSProperties = {
    background: `linear-gradient(${angle}deg, ${l.grade_cool_color ?? "#2a3f63"} 0%, ${
      l.grade_warm_color ?? "#ffb45e"
    } 100%)`,
    mixBlendMode: "color",
    opacity: String(strength),
  };
  return breathing ? breathe(style, l, strength) : { style };
}

function buildVignette(l: LightingParams, breathing: boolean): LightingLayer {
  const strength = clamp01(l.vignette_strength ?? 0.45);
  const style: CSSProperties = {
    background: `radial-gradient(ellipse at center, transparent ${l.vignette_size ?? 55}%, #000 100%)`,
    mixBlendMode: "multiply",
    opacity: String(strength),
  };
  return breathing ? breathe(style, l, strength) : { style };
}

/** bloom：背景副本整体模糊后提亮，用 screen 叠回去——亮的地方先溢出。 */
function buildBloom(l: LightingParams, breathing: boolean): LightingLayer {
  const radius = Math.max(1, l.bloom_radius ?? 18);
  const intensity = clamp01(l.bloom_intensity ?? 0.35);
  const style: CSSProperties = {
    filter: `blur(${radius}px) brightness(1.7) saturate(1.1)`,
    // 模糊会在四边吃出一圈透明，稍微放大盖住
    transform: `scale(${1 + Math.min(0.08, radius / 260)})`,
    mixBlendMode: "screen",
    opacity: String(intensity),
  };
  return breathing ? breathe(style, l, intensity) : { style };
}

export interface LightingPlan {
  characterFilter?: string;
  backgroundFilter?: string;
  bgOverlay?: CSSProperties;
  stageOverlay?: CSSProperties;
  bloom?: LightingLayer;
  directional?: { lit: LightingLayer; shadow: LightingLayer };
  grade?: LightingLayer;
  vignette?: LightingLayer;
}

/**
 * 把生效参数换算成各层 style。
 *
 * `null`（总开关关闭、或场景本就无灯光）返回空计划；此时各组件一律不挂载光影层。
 */
export function planLighting(l: LightingParams | null): LightingPlan {
  if (!l) return {};
  // 呼吸只作用在真正启用的层上；任一层开着呼吸就得带动画参数。
  const breathing = !!(
    l.breathing_enabled &&
    (l.directional_enabled || l.bloom_enabled || l.grade_enabled || l.vignette_enabled)
  );
  const plan: LightingPlan = {
    characterFilter: buildFilter(l.character),
    backgroundFilter: buildFilter(l.background),
    bgOverlay: buildOverlay(l, "background"),
    stageOverlay: buildOverlay(l, "character"),
  };
  if (l.bloom_enabled) plan.bloom = buildBloom(l, breathing);
  if (l.directional_enabled) plan.directional = buildDirectional(l, breathing);
  if (l.grade_enabled) plan.grade = buildGrade(l, breathing);
  if (l.vignette_enabled) plan.vignette = buildVignette(l, breathing);
  return plan;
}
