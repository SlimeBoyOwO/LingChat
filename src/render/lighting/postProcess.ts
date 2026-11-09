/**
 * 2.5D 多光源渲染内核 —— 后处理 pass（泛光 + 调色 + 最终合成）。
 *
 * 泛光走「亮部提取 → 两级不同尺度模糊 → screen 叠加」：
 * 小半径那层给近处高光描边，大半径那层铺开成雾状光晕。两级都从同一张亮部图
 * 出发，所以只在第一级做阈值提取，第二级直接模糊第一级的输出，省一次采样。
 *
 * 最终合成负责四件事，顺序不能乱：
 *   1. 泛光叠回场景（screen 混合）
 *   2. 调色 —— 饱和度 / 对比度 / 色调偏移（对应旧 CSS filter 的
 *      saturation / contrast / sepia，但作用于整幅合成结果）
 *   3. overlay 径向渐变异色层 —— 旧界面那套双色光晕 + mix-blend-mode，
 *      2.5D 路径此前完全没消费它，这里补上
 *   4. 去色（mono），放在最后，保证 mono = 1 时确实是全黑白
 */

import { COMMON_GLSL } from "./glsl";

/**
 * 亮部提取。阈值附近用二次软膝过渡，避免硬阈值把渐变边缘切成台阶。
 *
 * `uGlowColor` 给泛光染色：旧参数里的「发光色」在 CSS 路径上是 drop-shadow 的颜色，
 * 到 2.5D 路径对应的就是泛光本身的着色，白 = 不染色。
 */
export const BLOOM_PREFILTER_FRAGMENT_SHADER =
  COMMON_GLSL +
  `
uniform sampler2D uSceneTex;
uniform float uThresh;
uniform vec3  uGlowColor;

void main() {
  vec3 c = texture2D(uSceneTex, vFboUv).rgb;
  float br = max(max(c.r, c.g), c.b) * 0.52 + luma709(c) * 0.48;
  float knee = uThresh * 0.45 + 0.06;
  float soft = clamp(br - uThresh + knee, 0.0, 2.0 * knee);
  soft = (soft * soft) / (4.0 * knee + 1e-4);
  float contrib = max(soft, br - uThresh) / max(br, 1e-4);
  gl_FragColor = vec4(c * clamp(contrib, 0.0, 1.5) * uGlowColor, 1.0);
}`;

/**
 * 最终合成。
 *
 * overlay 用 `vUv`（y 向下、和旧 `light_x` / `light_y` 的百分数同向），
 * 不能用 `vFboUv`（那是翻转后的 FBO 坐标），否则光晕会上下颠倒。
 */
export const FINAL_COMPOSITE_FRAGMENT_SHADER =
  COMMON_GLSL +
  `
uniform sampler2D uSceneTex;
uniform sampler2D uBloom1Tex;
uniform sampler2D uBloom2Tex;
uniform float uBloomInt;
uniform float uMono;

uniform float uSaturation;
uniform float uContrast;
uniform vec3  uTintColor;
uniform float uTintAmount;

uniform float uOverlayEnabled;
uniform vec4  uOverlayColor1;   // rgb = 中心色，a = 不透明度
uniform vec4  uOverlayColor2;   // rgb = 边缘色
uniform vec2  uOverlayCenter;
uniform float uOverlayRadius;
uniform float uOverlayMode;
uniform float uStageAspect;

/**
 * CSS mix-blend-mode 的子集。编码在 JS 侧映射：
 * 0 normal / 1 multiply / 2 screen / 3 overlay / 4 soft-light / 5 darken / 6 lighten
 */
vec3 blendMode(vec3 base, vec3 src, float mode) {
  if (mode < 0.5) return src;
  if (mode < 1.5) return base * src;
  if (mode < 2.5) return 1.0 - (1.0 - base) * (1.0 - src);
  if (mode < 3.5) {
    return mix(2.0 * base * src, 1.0 - 2.0 * (1.0 - base) * (1.0 - src), step(0.5, base));
  }
  if (mode < 4.5) return pegtopSoftLight(base, src);
  if (mode < 5.5) return min(base, src);
  return max(base, src);
}

void main() {
  vec3 scene = texture2D(uSceneTex, vFboUv).rgb;
  vec3 b1 = texture2D(uBloom1Tex, vFboUv).rgb;
  vec3 b2 = texture2D(uBloom2Tex, vFboUv).rgb;
  vec3 bloom = clamp((b1 * 0.48 + b2 * 0.72) * uBloomInt * 1.25, 0.0, 0.94);
  vec3 col = 1.0 - (1.0 - scene) * (1.0 - bloom);

  // 饱和度：绕亮度轴缩放。saturation = 1 原样，0 灰度，>1 增强
  float l = luma709(col);
  col = mix(vec3(l), col, uSaturation);

  // 对比度：绕中灰拉伸
  col = (col - 0.5) * uContrast + 0.5;

  // 色调偏移：乘性偏色，uTintColor 已归一化到最大分量为 1
  col = col * mix(vec3(1.0), uTintColor, uTintAmount);

  // overlay 径向渐变
  if (uOverlayEnabled > 0.5) {
    vec2 d = vUv - uOverlayCenter;
    d.x *= uStageAspect;
    float t = smoothstep(0.0, 1.0, clamp(length(d) / max(uOverlayRadius, 1e-4), 0.0, 1.0));
    vec3 ovCol = mix(uOverlayColor1.rgb, uOverlayColor2.rgb, t);
    col = mix(col, blendMode(col, ovCol, uOverlayMode), uOverlayColor1.a);
  }

  // 去色放最后，保证 mono = 1 时确实全黑白
  col = mix(col, vec3(luma709(col)), uMono);

  gl_FragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}`;
