/**
 * 2.5D 多光源渲染内核。
 *
 * 分两层：
 * - `BackgroundLightingRenderer`：只画背景，直接输出到屏幕（第一层，最小可用）
 * - `SceneLightingRenderer`：背景 → 离屏 → 模糊 → 与立绘合成（第二层，完整场景）
 *
 * 两层共用同一套 GLSL 与灯光打包逻辑，数值口径一致。都还没接进游戏舞台，
 * 挂载属于后续 PR。
 */

export { BackgroundLightingRenderer } from "./LightingRenderer";
export type { BackgroundLightingInput, BackgroundRect, StageSize } from "./LightingRenderer";

export { SceneLightingRenderer, MAX_CHARACTERS } from "./SceneLightingRenderer";
export type {
  CharacterMaps,
  CharacterLightingSet,
  Rect,
  SceneLightingFrame,
} from "./SceneLightingRenderer";

export { createFlatNormalTexture, generatePseudoDepth, generatePseudoNormal } from "./pseudoDepth";

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

export { BLUR_FRAGMENT_SHADER, CHARACTER_FRAGMENT_SHADER } from "./characterShader";
