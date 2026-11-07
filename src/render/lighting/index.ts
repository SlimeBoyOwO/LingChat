/**
 * 2.5D 多光源渲染内核（第一层：背景 pass）。
 *
 * 用法：
 * ```ts
 * const renderer = new BackgroundLightingRenderer(canvas);
 * renderer.resize(w, h);
 * renderer.setBackground(imageBitmap);
 * renderer.render({
 *   lights: sceneLighting.lights ?? [],
 *   global: sceneLighting.global,
 *   stage: { width: w, height: h },
 *   backgroundRect: [0, 0, w, h],
 * });
 * ```
 */

export { BackgroundLightingRenderer } from "./LightingRenderer";
export type { BackgroundLightingInput, BackgroundRect, StageSize } from "./LightingRenderer";

export {
  LIGHT_TYPE_CODE,
  MAX_LIGHTS,
  dirVector,
  hexToLinear,
  normalizeHex,
  packLights,
} from "./lights";
export type { PackedLights } from "./lights";

export {
  BACKGROUND_FRAGMENT_SHADER,
  COMMON_GLSL,
  LIGHT_UNIFORMS_GLSL,
  VERTEX_SHADER,
} from "./glsl";
