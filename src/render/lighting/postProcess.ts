/**
 * 2.5D 多光源渲染内核 —— 后处理 pass（泛光 + 最终合成）。
 *
 * 泛光走「亮部提取 → 两级不同尺度模糊 → screen 叠加」：
 * 小半径那层给近处高光描边，大半径那层铺开成雾状光晕。两级都从同一张亮部图
 * 出发，所以只在第一级做阈值提取，第二级直接模糊第一级的输出，省一次采样。
 *
 * 最终合成只做两件事：把泛光叠回场景（screen 混合），以及按 `mono` 去色。
 */

import { COMMON_GLSL } from "./glsl";

/**
 * 亮部提取。阈值附近用二次软膝过渡，避免硬阈值把渐变边缘切成台阶。
 */
export const BLOOM_PREFILTER_FRAGMENT_SHADER =
  COMMON_GLSL +
  `
uniform sampler2D uSceneTex;
uniform float uThresh;

void main() {
  vec3 c = texture2D(uSceneTex, vFboUv).rgb;
  float br = max(max(c.r, c.g), c.b) * 0.52 + luma709(c) * 0.48;
  float knee = uThresh * 0.45 + 0.06;
  float soft = clamp(br - uThresh + knee, 0.0, 2.0 * knee);
  soft = (soft * soft) / (4.0 * knee + 1e-4);
  float contrib = max(soft, br - uThresh) / max(br, 1e-4);
  gl_FragColor = vec4(c * clamp(contrib, 0.0, 1.5), 1.0);
}`;

/**
 * 最终合成：场景 + 两级泛光，最后按需要去色。
 */
export const FINAL_COMPOSITE_FRAGMENT_SHADER =
  COMMON_GLSL +
  `
uniform sampler2D uSceneTex;
uniform sampler2D uBloom1Tex;
uniform sampler2D uBloom2Tex;
uniform float uBloomInt;
uniform float uMono;

void main() {
  vec3 scene = texture2D(uSceneTex, vFboUv).rgb;
  vec3 b1 = texture2D(uBloom1Tex, vFboUv).rgb;
  vec3 b2 = texture2D(uBloom2Tex, vFboUv).rgb;
  vec3 bloom = clamp((b1 * 0.48 + b2 * 0.72) * uBloomInt * 1.25, 0.0, 0.94);
  vec3 outRgb = 1.0 - (1.0 - scene) * (1.0 - bloom);
  outRgb = mix(outRgb, vec3(luma709(outRgb)), uMono);
  gl_FragColor = vec4(clamp(outRgb, 0.0, 1.0), 1.0);
}`;
