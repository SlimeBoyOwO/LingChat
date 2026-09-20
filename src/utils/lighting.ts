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

/**
 * 旧版的径向光照层：`overlay_target` 决定它压在背景上、角色上还是两者。
 *
 * 色标不是从内到外线性过渡，而是走到半径的 30% 就衰减掉大半——真实灯光按距离衰减，
 * 中心亮、边缘很快没了。线性过渡配 `screen` 时，半径一大就等于整幅画面均匀加一层白雾。
 */
function buildOverlay(
  l: LightingParams,
  want: "background" | "character"
): CSSProperties | undefined {
  if (!l.overlay_enabled) return undefined;
  if (l.overlay_target !== "both" && l.overlay_target !== want) return undefined;
  const blend = l.blend_mode !== "normal" ? l.blend_mode : "overlay";
  const r = l.overlay_radius;
  const c1 = l.overlay_color1;
  const c2 = l.overlay_color2;
  return {
    background:
      `radial-gradient(circle at ${l.light_x}% ${l.light_y}%, ` +
      `${c1} 0%, ` +
      `color-mix(in srgb, ${c1} 55%, ${c2}) ${Math.round(r * 0.3)}%, ` +
      `color-mix(in srgb, ${c1} 16%, ${c2}) ${Math.round(r * 0.62)}%, ` +
      `${c2} ${r}%)`,
    mixBlendMode: blend as CSSProperties["mixBlendMode"],
    opacity: String(l.overlay_opacity),
  };
}

/**
 * 方向光：两层线性渐变，受光侧 `screen` 提亮、背光侧 `multiply` 压暗。
 *
 * `light_angle` 沿用 CSS 的罗盘约定（0deg = 光源在正上方，顺时针增大），必须和
 * `light_x/light_y` 指到同一个位置——否则光斑在窗边、阴影也压在窗边。
 *
 * 两层不共用同一个不透明度。`light_strength` 表达的是「这束光的明暗对比有多强」，
 * 落地时背光层按原值压暗，受光层只取 45%：暖色本身接近纯白，`screen` 叠在已经亮的
 * 地方会直接糊成白雾，而 `multiply` 叠在暗处只是变深。同一个 alpha 下提亮比压暗显眼
 * 得多，所以两边必须不对称。
 */
function buildDirectional(l: LightingParams, breathing: boolean) {
  const angle = l.light_angle ?? 315;
  const softness = clamp01(l.light_softness ?? 0.55);
  const strength = clamp01(l.light_strength ?? 0.5);
  const litStrength = clamp01(strength * 0.45);
  // 过渡带宽度：柔和度 0 → 明暗在中线硬切；1 → 渐变铺满几乎整个画面
  const spread = 25 + softness * 65;
  const lit = {
    background: `linear-gradient(${angle}deg, transparent ${100 - spread}%, ${
      l.light_warm_color ?? "#ffd9a0"
    } 100%)`,
    mixBlendMode: "screen" as const,
    opacity: String(litStrength),
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
    lit: withBreath(lit, litStrength),
    shadow: withBreath(shadow, strength),
  };
}

/**
 * 冷暖分离：`mix-blend-mode: soft-light` 沿明暗重新上色——亮部偏暖、暗部偏冷，
 * 亮度关系基本保留。用 `color` 会把整幅画的色相饱和度一起换掉，出来是灰蒙蒙的
 * 一层染色，看不出「光」只看得出「滤镜」。
 */
function buildGrade(l: LightingParams, breathing: boolean): LightingLayer {
  const angle = l.light_angle ?? 315;
  const strength = clamp01(l.grade_strength ?? 0.35);
  const style: CSSProperties = {
    background: `linear-gradient(${angle}deg, ${l.grade_cool_color ?? "#2a3f63"} 0%, ${
      l.grade_warm_color ?? "#ffb45e"
    } 100%)`,
    mixBlendMode: "soft-light",
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

/**
 * bloom：背景副本模糊后用 screen 叠回去。
 *
 * 关键是先「压出亮部」再模糊。直接 `brightness(1.7)` 会把暗部一起抬起来，screen
 * 叠回去等于给整幅画蒙一层灰雾——暗处不再黑，画面立刻发闷。
 *
 * 顺序是 `brightness` 再 `contrast`：先把整幅压暗一点，让中间调落到 `contrast` 的
 * 0.5 轴心以下，`contrast` 放大明暗差时就会把它们剪到接近黑。screen 对黑色恒等，
 * 于是只有真正的高光（窗、灯、发丝亮边）会溢出光，而不是一片白雾。
 */
function buildBloom(l: LightingParams, breathing: boolean): LightingLayer {
  const radius = Math.max(1, l.bloom_radius ?? 18);
  const intensity = clamp01(l.bloom_intensity ?? 0.35);
  const style: CSSProperties = {
    filter: `brightness(0.62) contrast(4.5) blur(${radius}px) saturate(1.3)`,
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
