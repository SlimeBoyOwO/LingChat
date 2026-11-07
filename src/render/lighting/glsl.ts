/**
 * 2.5D 多光源渲染内核 —— GLSL 源码。
 *
 * 逐像素光照，与 `GameBackground.vue` 里的 CSS `filter` / `radial-gradient`
 * 是两条并存的路径：旧场景存档只有 CSS 字段，新场景带 WebGL 参数。
 *
 * 这里只做「背景 pass」：把背景图当作完全漫反射的受光面，按 8 盏灯的
 * 类型（点光 / 环境底光 / 平行光 / 锥光）累加线性光强，再做软膝色调映射
 * 与暗部抬升。角色材质（伪法线 / 阶梯明暗 / 边缘光）属于下一个 PR。
 *
 * 数值常量与设计稿沙箱（`workbench.template.html`）逐字对应，改动前先
 * 在沙箱里验证效果，避免两边漂移。
 */

/** 全屏四边形顶点着色器；`vUv` 是舞台 UV（y 向下），`vFboUv` 是纹理 UV（y 向上）。 */
export const VERTEX_SHADER = `
attribute vec2 aPos;
varying vec2 vUv;
varying vec2 vFboUv;
void main() {
  vUv = vec2(aPos.x * 0.5 + 0.5, 0.5 - aPos.y * 0.5);
  vFboUv = aPos * 0.5 + 0.5;
  gl_Position = vec4(aPos, 0.0, 1.0);
}`;

/** 公用的色彩空间与混色工具。 */
export const COMMON_GLSL = `
precision highp float;
varying vec2 vUv;
varying vec2 vFboUv;

vec3 decodeSrgb(vec3 c) {
  return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(vec3(0.04045), c));
}
vec3 encodeSrgb(vec3 c) {
  c = max(vec3(0.0), c);
  return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(vec3(0.0031308), c));
}
vec3 pegtopSoftLight(vec3 base, vec3 blend) {
  return (1.0 - 2.0 * blend) * base * base + 2.0 * blend * base;
}
float luma709(vec3 c) {
  return dot(c, vec3(0.2126, 0.7152, 0.0722));
}
`;

/**
 * 8 盏灯的 uniform 声明。灯型编码见 `lights.ts` 的 `LIGHT_TYPE_CODE`：
 * 0 = point，1 = ambient，2 = directional，3 = spot。
 */
export const LIGHT_UNIFORMS_GLSL = `
uniform int   uLCount;
uniform int   uKeyIndex;
uniform int   uLType[8];
uniform vec3  uLColor[8];
uniform float uLInt[8];
uniform vec3  uLPos[8];
uniform float uLRad[8];
uniform float uLSoft[8];
uniform vec3  uLAxis[8];
uniform float uLCos[8];
uniform float uLPen[8];
`;

/** 背景片元着色器：多光源逐像素着色 + 软膝色调映射 + 暗部抬升。 */
export const BACKGROUND_FRAGMENT_SHADER =
  COMMON_GLSL +
  LIGHT_UNIFORMS_GLSL +
  `
uniform sampler2D uBgTex;
uniform vec2  uStageSize;
uniform vec4  uBgRect;
uniform float uSoftKnee;
uniform float uLift;
uniform vec3  uBgAvgSrgb;

void main() {
  vec2 stagePx = vUv * uStageSize;
  vec2 localUv = (stagePx - uBgRect.xy) / uBgRect.zw;
  if (localUv.x < 0.0 || localUv.x > 1.0 || localUv.y < 0.0 || localUv.y > 1.0) {
    gl_FragColor = vec4(0.0392, 0.0392, 0.0392, 1.0);
    return;
  }
  vec4 tex = texture2D(uBgTex, localUv);
  if (uLCount == 0) {
    gl_FragColor = vec4(tex.rgb, 1.0);
    return;
  }
  vec3 srcLin = decodeSrgb(tex.rgb);
  vec3 P = vec3(stagePx, 0.0);
  vec3 ambAcc = vec3(0.0);
  vec3 litAcc = vec3(0.0);

  for (int i = 0; i < 8; i++) {
    if (i >= uLCount) break;
    int t = uLType[i];
    vec3 lc = uLColor[i] * uLInt[i];
    if (t == 1) {
      ambAcc += lc;
      continue;
    }
    vec3 L;
    float atten = 1.0;
    if (t == 0 || t == 3) {
      vec3 toL = uLPos[i] - P;
      float d = length(toL.xy);
      L = normalize(toL);
      float inner = uLRad[i] * (1.0 - uLSoft[i] * 0.96);
      float tt = clamp((d - inner) / max(1.0, uLRad[i] - inner), 0.0, 1.0);
      float s1 = 1.0 - tt * tt * (3.0 - 2.0 * tt);
      float s2 = 1.0 - tt * tt * tt * (tt * (tt * 6.0 - 15.0) + 10.0);
      atten = mix(s2, s1, 0.45);
      if (t == 3) {
        vec2 dirToPix2D = P.xy - uLPos[i].xy;
        float d2D = length(dirToPix2D);
        if (d2D > 1e-4) {
          vec2 dirNorm2D = dirToPix2D / d2D;
          vec2 axisNorm2D = (length(uLAxis[i].xy) > 1e-4) ? normalize(uLAxis[i].xy) : vec2(1.0, 0.0);
          float cosA = dot(dirNorm2D, axisNorm2D);
          atten *= smoothstep(uLCos[i], uLPen[i], cosA);
        } else {
          atten = 0.0;
        }
      }
    } else {
      L = uLAxis[i];
    }
    float ndotl = clamp(L.z, 0.0, 1.0);
    float shadeK = (t == 2) ? mix(0.45, 1.15, ndotl) : mix(0.42, 1.08, ndotl);
    litAcc += lc * atten * shadeK;
  }

  vec3 gain = max(vec3(0.0), ambAcc + litAcc);
  vec3 baseDarken = srcLin * min(gain * 1.18, vec3(1.0));
  vec3 overGain = max(vec3(0.0), gain * 1.18 - vec3(1.0));
  vec3 baseSrgb = encodeSrgb(baseDarken);
  vec3 overSrgb = clamp(encodeSrgb(overGain * 0.42), 0.0, 1.0);
  vec3 softTinted = pegtopSoftLight(baseSrgb, clamp(vec3(0.5) + overSrgb * 0.45, 0.0, 1.0));
  vec3 screenLift = 1.0 - exp(-overGain * (0.45 - 0.20 * uSoftKnee));
  vec3 outSrgb = 1.0 - (1.0 - mix(baseSrgb, softTinted, 0.65)) * (1.0 - clamp(encodeSrgb(screenLift) * 0.42, 0.0, 0.85));

  vec3 ambSrgb = encodeSrgb(ambAcc);
  vec3 envTint = mix(uBgAvgSrgb, ambSrgb, step(0.01, luma709(ambSrgb)) * 0.65);
  float shadowMask = 1.0 - smoothstep(0.0, 0.52, luma709(outSrgb));
  outSrgb = clamp(outSrgb + uLift * shadowMask * mix(vec3(0.14), envTint * 0.36, 0.82) * (1.0 - outSrgb), 0.0, 1.0);
  gl_FragColor = vec4(outSrgb, 1.0);
}`;
