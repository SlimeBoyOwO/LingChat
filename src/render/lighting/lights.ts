/**
 * 把 `LightSource[]` 打包成 WebGL uniform 需要的定型数组。
 *
 * 全部是纯函数，不碰 DOM / WebGL，便于单独测试。数值口径与沙箱
 * （`workbench.template.html` 的 `uploadLightUniforms`）逐项对齐：
 *
 * - 位置：`x * 舞台宽`、`y * 舞台高`（像素），`z * min(宽, 高)` 作为高度
 * - 半径：`radius * min(宽, 高)`，保证不同宽高比下光斑不被拉扁
 * - 颜色：hex 先转线性空间再乘主强度，避免在 sRGB 空间里做加法偏亮
 * - 锥光：`cone` / `penumbra` 以角度存，这里换算成余弦交给 `smoothstep`
 */

import type { LightSource, LightType } from "../../api/services/scene";

/** 灯型编码，与 `glsl.ts` 的 `uLType` 判断保持一致。 */
export const LIGHT_TYPE_CODE: Record<LightType, number> = {
  point: 0,
  ambient: 1,
  directional: 2,
  spot: 3,
};

/** 着色器里的固定槽位与业务上限。 */
export const MAX_LIGHTS = 8;

/** 打包结果，字段名对应 `glsl.ts` 的 `uL*` uniform。 */
export interface PackedLights {
  /** 实际参与计算的灯数（已过滤 `enabled === false`，且不超过 `MAX_LIGHTS`）。 */
  count: number;
  /** 主光槽位；没有标记主光时为 -1。 */
  keyIndex: number;
  types: Int32Array;
  colors: Float32Array;
  intens: Float32Array;
  pos: Float32Array;
  rads: Float32Array;
  softs: Float32Array;
  axes: Float32Array;
  cos: Float32Array;
  pen: Float32Array;
}

const DEG = Math.PI / 180;

/** 把 `#abc` / `abc` / `#AABBCC` 统一成小写六位十六进制；非法输入回落白色。 */
export function normalizeHex(hex: string): string {
  if (typeof hex !== "string") return "#ffffff";
  let h = hex.trim().replace(/^#/, "");
  if (h.length === 3) {
    h = h
      .split("")
      .map((c) => c + c)
      .join("");
  }
  if (!/^[0-9a-fA-F]{6}$/.test(h)) return "#ffffff";
  return "#" + h.toLowerCase();
}

/** sRGB 分量转线性空间。 */
function srgbToLinear(v: number): number {
  return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
}

/** `#rrggbb` 转线性 RGB 三元组。 */
export function hexToLinear(hex: string): [number, number, number] {
  const h = normalizeHex(hex);
  return [1, 3, 5].map((i) => srgbToLinear(parseInt(h.slice(i, i + 2), 16) / 255)) as [
    number,
    number,
    number,
  ];
}

/** 方位角 / 仰角（度）转单位方向向量。方位角自 +x 起逆时针，仰角自水平面起。 */
export function dirVector(azimuthDeg: number, elevationDeg: number): [number, number, number] {
  const az = azimuthDeg * DEG;
  const el = elevationDeg * DEG;
  return [Math.cos(az) * Math.cos(el), Math.sin(az) * Math.cos(el), Math.sin(el)];
}

/**
 * 打包灯光。
 *
 * @param lights    场景里的全部灯，函数内部会跳过 `enabled === false` 的项
 * @param frameW    舞台像素宽
 * @param frameH    舞台像素高
 * @param masterMul 全局主强度（`global.master`），乘进颜色
 */
export function packLights(
  lights: readonly LightSource[],
  frameW: number,
  frameH: number,
  masterMul = 1,
): PackedLights {
  const active = lights.filter((l) => l.enabled !== false).slice(0, MAX_LIGHTS);
  const minSide = Math.min(frameW, frameH);

  const types = new Int32Array(MAX_LIGHTS);
  const colors = new Float32Array(MAX_LIGHTS * 3);
  const intens = new Float32Array(MAX_LIGHTS);
  const pos = new Float32Array(MAX_LIGHTS * 3);
  const rads = new Float32Array(MAX_LIGHTS);
  const softs = new Float32Array(MAX_LIGHTS);
  const axes = new Float32Array(MAX_LIGHTS * 3);
  const cos = new Float32Array(MAX_LIGHTS);
  const pen = new Float32Array(MAX_LIGHTS);

  let keyIndex = -1;
  active.forEach((l, i) => {
    if (keyIndex === -1 && l.is_key) keyIndex = i;

    types[i] = LIGHT_TYPE_CODE[l.type] ?? LIGHT_TYPE_CODE.point;

    const rgb = hexToLinear(l.color);
    colors[i * 3] = rgb[0] * masterMul;
    colors[i * 3 + 1] = rgb[1] * masterMul;
    colors[i * 3 + 2] = rgb[2] * masterMul;

    intens[i] = l.intensity;

    const dir = dirVector(l.azimuth, l.elevation);
    axes[i * 3] = dir[0];
    axes[i * 3 + 1] = dir[1];
    axes[i * 3 + 2] = dir[2];

    const halfCone = l.cone * 0.5 * DEG;
    cos[i] = Math.cos(halfCone);
    const coreDeg = l.cone * 0.5 * Math.max(0.02, 1 - (l.penumbra || 0) * 0.96);
    pen[i] = Math.cos(coreDeg * DEG);

    softs[i] = l.softness;
    rads[i] = l.radius * minSide;
    pos[i * 3] = l.x * frameW;
    pos[i * 3 + 1] = l.y * frameH;
    pos[i * 3 + 2] = l.z * minSide;
  });

  return {
    count: active.length,
    keyIndex,
    types,
    colors,
    intens,
    pos,
    rads,
    softs,
    axes,
    cos,
    pen,
  };
}
