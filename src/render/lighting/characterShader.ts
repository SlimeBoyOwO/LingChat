/**
 * 2.5D 多光源渲染内核 —— 角色 pass 与模糊 pass 的 GLSL。
 *
 * 角色片元着色器是整条管线里最重的一段：它要把「立绘（带 alpha）+ 已着色的
 * 背景 + 模糊背景（用于光包裹）+ 伪法线 + 伪深度」合成出来，并做：
 *
 * 1. 逐灯散射：`half-Lambert` 经卡通阶梯量化后乘自阴影
 * 2. 高光：Blinn-Phong，按阶梯明暗的落点收窄
 * 3. 轮廓光：由立绘 alpha 的梯度求轮廓法线，与表面 Fresnel 融合
 * 4. 环境同化：用背景平均色与灯光环境项给立绘整体染色
 * 5. 光包裹：把背景模糊一版，沿主光方向偏移后按 alpha 内缘叠加
 *
 * 数值口径与设计稿沙箱（`workbench.template.html`）一致，改之前先在沙箱里验证。
 */

import { COMMON_GLSL, LIGHT_UNIFORMS_GLSL } from "./glsl";

/** 角色合成片元着色器。 */
export const CHARACTER_FRAGMENT_SHADER =
  COMMON_GLSL +
  LIGHT_UNIFORMS_GLSL +
  `
uniform sampler2D uCharTex;
uniform sampler2D uBgSceneTex;
uniform sampler2D uBgWrapTex;
uniform sampler2D uNormalTex;
uniform sampler2D uDepthTex;
uniform float uHasNormal;
uniform float uHasDepth;
uniform float uNormalStrength;
uniform float uCelThresh;
uniform float uCelSoftness;
uniform float uSpecInt;
uniform float uSpecGloss;
uniform float uSelfShadowInt;
uniform vec2  uStageSize;
uniform vec4  uCharRect;
uniform float uAlbedoProtect;
uniform float uSoftKnee;
uniform float uLift;
uniform float uAmbientBlend;
uniform float uLightWrap;
uniform float uRimWidth;
uniform float uCharOnly;
uniform vec3  uBgAvgSrgb;
/** 立绘单独曝光增益（0–2），只作用于角色像素，不影响背景 */
uniform float uCharExposure;

float sampleCharAlpha(vec2 uv) {
  if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) return 0.0;
  return texture2D(uCharTex, uv).a;
}

void main() {
  vec3 bgScene = (uCharOnly > 0.5) ? vec3(0.0) : texture2D(uBgSceneTex, vFboUv).rgb;
  vec2 stagePx = vUv * uStageSize;
  vec2 localUv = (stagePx - uCharRect.xy) / uCharRect.zw;

  if (localUv.x < -0.08 || localUv.x > 1.08 || localUv.y < -0.08 || localUv.y > 1.08) {
    gl_FragColor = (uCharOnly > 0.5) ? vec4(0.0) : vec4(bgScene, 1.0);
    return;
  }

  vec4 tex = (localUv.x >= 0.0 && localUv.x <= 1.0 && localUv.y >= 0.0 && localUv.y <= 1.0)
    ? texture2D(uCharTex, localUv) : vec4(0.0);

  if (uLCount == 0) {
    if (uCharOnly > 0.5) {
      gl_FragColor = vec4(tex.rgb * tex.a, tex.a);
    } else {
      gl_FragColor = vec4(mix(bgScene, tex.rgb, tex.a), 1.0);
    }
    return;
  }

  vec3 P = vec3(stagePx, 0.0);
  vec2 domDir2D = vec2(0.0);
  float domWeight = 0.001;
  for (int i = 0; i < 8; i++) {
    if (i >= uLCount) break;
    int t = uLType[i];
    if (t == 1) continue;
    vec2 dir2 = (t == 2) ? uLAxis[i].xy : normalize(uLPos[i].xy - P.xy + vec2(1e-4));
    float w = uLInt[i] * luma709(uLColor[i]);
    if (i == uKeyIndex) w *= 4.0;
    domDir2D += dir2 * w;
    domWeight += w;
  }
  domDir2D = domDir2D / domWeight;
  float domLen = length(domDir2D);
  if (domLen > 1.0) domDir2D /= domLen;

  float minSide = min(uStageSize.x, uStageSize.y);
  float rimPx = max(1.0, uRimWidth * (minSide / 720.0));
  vec2 rimStep = vec2(rimPx) / max(vec2(1.0), uCharRect.zw);
  vec2 dirBias = -domDir2D * rimStep * 0.42;

  float isoAlphaSum = 0.0;
  float dirAlphaSum = 0.0;
  for (int k = 0; k < 6; k++) {
    float a1 = float(k) * 1.04719755;
    float a2 = a1 + 0.52359878;
    vec2 d1 = vec2(cos(a1), sin(a1)) * 0.46;
    vec2 d2 = vec2(cos(a2), sin(a2));
    isoAlphaSum += sampleCharAlpha(localUv + d1 * rimStep) + sampleCharAlpha(localUv + d2 * rimStep);
    dirAlphaSum += sampleCharAlpha(localUv + dirBias + d1 * rimStep) + sampleCharAlpha(localUv + dirBias + d2 * rimStep);
  }
  float blurAlphaIso = isoAlphaSum / 12.0;
  float blurAlphaDir = dirAlphaSum / 12.0;

  vec2 fboLightShift = vec2(domDir2D.x, -domDir2D.y) * 0.018;
  vec3 bgWrapRgb = texture2D(uBgWrapTex, clamp(vFboUv + fboLightShift, 0.002, 0.998)).rgb;

  if (tex.a <= 0.002) {
    if (uCharOnly > 0.5) {
      gl_FragColor = vec4(0.0);
      return;
    }
    float outerMask = pow(clamp(blurAlphaIso * 1.45, 0.0, 1.0), 1.18) * uLightWrap * 0.64;
    vec3 haloTint = mix(bgWrapRgb, vec3(1.0, 0.97, 0.92), 0.32);
    vec3 outerCol = 1.0 - (1.0 - bgScene) * (1.0 - clamp(haloTint * outerMask * 1.18, 0.0, 0.78));
    gl_FragColor = vec4(outerCol, 1.0);
    return;
  }

  vec2 gradStep = max(rimStep * 0.55, vec2(1.5) / max(vec2(1.0), uCharRect.zw));
  float gx = sampleCharAlpha(localUv + vec2(gradStep.x, 0.0)) - sampleCharAlpha(localUv - vec2(gradStep.x, 0.0));
  float gy = sampleCharAlpha(localUv + vec2(0.0, gradStep.y)) - sampleCharAlpha(localUv - vec2(0.0, gradStep.y));
  vec2 contourOut = -vec2(gx, gy);
  float cLen = length(contourOut);
  vec2 contourN = (cLen > 1e-3) ? (contourOut / cLen) : vec2(0.0, -1.0);
  float edgeBand = clamp(1.0 - blurAlphaIso, 0.0, 1.0);

  vec3 surfaceN = vec3(0.0, 0.0, 1.0);
  if (uHasNormal > 0.5) {
    vec4 normSample = texture2D(uNormalTex, localUv);
    vec3 rawN = normSample.rgb * 2.0 - 1.0;
    surfaceN = normalize(vec3(rawN.xy * uNormalStrength, rawN.z));
  }

  float pixelDepth = 0.5;
  if (uHasDepth > 0.5) {
    pixelDepth = texture2D(uDepthTex, localUv).r;
  }

  vec3 ambAcc = vec3(0.0);
  vec3 washAcc = vec3(0.0);
  vec3 rimAcc = vec3(0.0);
  vec2 relSprite = localUv - vec2(0.5, 0.44);

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
      float effRad = uLRad[i];
      float inner = effRad * (1.0 - uLSoft[i] * 0.96);
      float tt = clamp((d - inner) / max(1.0, effRad - inner), 0.0, 1.0);
      atten = 1.0 - tt * tt * (3.0 - 2.0 * tt);
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

    vec2 L2 = L.xy;
    float l2Len = length(L2);
    vec2 L2n = (l2Len > 1e-3) ? (L2 / l2Len) : vec2(0.0, -1.0);

    float acrossWash = clamp(0.64 + dot(relSprite, L2n) * 0.42 + L.z * 0.16, 0.24, 1.16);

    float ndotl = dot(surfaceN, L);
    float halfLambert = clamp(ndotl * 0.5 + 0.5, 0.0, 1.0);
    float halfSoft = max(0.005, uCelSoftness * 0.5);
    float celStep = smoothstep(uCelThresh - halfSoft, uCelThresh + halfSoft, halfLambert);
    float bodyDiffuse = mix(0.30, 1.15, celStep);

    float selfShadow = 1.0;
    if (uHasDepth > 0.5 && uSelfShadowInt > 0.02) {
      vec2 shadowStep = -L2n * (0.012 * uSelfShadowInt);
      float neighborDepth = texture2D(uDepthTex, clamp(localUv + shadowStep, 0.0, 1.0)).r;
      if (neighborDepth > pixelDepth + 0.035) {
        selfShadow = mix(1.0, 0.42, clamp((neighborDepth - pixelDepth) * 12.0, 0.0, 1.0));
      }
    }

    vec3 V = vec3(0.0, 0.0, 1.0);
    vec3 H = normalize(L + V);
    float ndoth = max(0.0, dot(surfaceN, H));
    float specTerm = pow(ndoth, max(4.0, uSpecGloss)) * smoothstep(0.12, 0.45, celStep) * uSpecInt;

    float volumeDiffuse = (bodyDiffuse * selfShadow) + specTerm;
    float finalDiffuse = mix(acrossWash, volumeDiffuse, uHasNormal);

    float contourRim = pow(max(0.0, dot(contourN, L2n)), 1.25) * (0.45 + 0.55 * (1.0 - clamp(L.z, 0.0, 1.0)));
    float fresnel = pow(1.0 - clamp(surfaceN.z, 0.0, 1.0), 2.2) * max(0.0, dot(surfaceN.xy, L2n));
    float finalRim = mix(contourRim, contourRim + fresnel * 0.45, uHasNormal);

    washAcc += lc * atten * finalDiffuse;
    rimAcc += lc * atten * (0.35 + 0.95 * finalRim);
  }

  vec3 srcSrgb = tex.rgb;
  vec3 srcLin = decodeSrgb(srcSrgb);
  float srcLuma = luma709(srcLin);

  vec3 ambSrgb = encodeSrgb(ambAcc);
  float hasAmb = step(0.005, luma709(ambSrgb));
  vec3 envRefSrgb = mix(uBgAvgSrgb, mix(uBgAvgSrgb, ambSrgb, 0.72), hasAmb);
  float envRefLuma = max(0.04, luma709(envRefSrgb));
  vec3 envNormTint = clamp(envRefSrgb / envRefLuma * 0.52, 0.08, 0.92);
  vec3 assimilatedSrgb = mix(srcSrgb, pegtopSoftLight(srcSrgb, envNormTint), uAmbientBlend * 0.88);
  vec3 assimilatedLin = decodeSrgb(assimilatedSrgb);

  vec3 totalGain = max(vec3(0.0), ambAcc + washAcc + rimAcc * edgeBand * 0.45);
  float gainLuma = luma709(totalGain);
  float envEnergy = smoothstep(0.015, 0.42, max(luma709(ambAcc) * 2.1, gainLuma * 0.78));

  float p = clamp(uAlbedoProtect, 0.0, 1.0);
  float skinMask = smoothstep(0.22, 0.65, srcLuma);
  float brightMask = smoothstep(0.60, 0.92, srcLuma);
  vec3 neutralGain = vec3(gainLuma);
  vec3 gainDesat = mix(totalGain, neutralGain * 1.02, p * mix(0.38, 0.72, brightMask));
  float dynFloor = envEnergy * mix(0.0, 0.24 + 0.48 * p, max(skinMask, brightMask * 0.85));
  vec3 effGain = max(gainDesat, vec3(dynFloor));

  vec3 baseModLin = assimilatedLin * min(effGain * 1.15, vec3(1.0));
  vec3 overLin = max(vec3(0.0), effGain * 1.15 - vec3(0.92));
  vec3 midSrgb = encodeSrgb(baseModLin);
  vec3 overTintSrgb = clamp(encodeSrgb(overLin * 0.45), 0.0, 1.0);
  vec3 softLitSrgb = pegtopSoftLight(midSrgb, clamp(vec3(0.5) + overTintSrgb * 0.46, 0.0, 1.0));
  vec3 kneeScreen = 1.0 - exp(-overLin * (0.62 - 0.34 * uSoftKnee));
  vec3 litSrgb = 1.0 - (1.0 - mix(midSrgb, softLitSrgb, 0.62)) * (1.0 - clamp(encodeSrgb(kneeScreen) * 0.52, 0.0, 0.88));

  float lumaS = luma709(litSrgb);
  float shadowMask = 1.0 - smoothstep(0.0, 0.58, lumaS);
  vec3 shadowTarget = mix(vec3(0.16, 0.165, 0.18), envRefSrgb * 0.56, 0.88);
  litSrgb = clamp(litSrgb + uLift * shadowMask * shadowTarget * (1.0 - litSrgb), 0.0, 1.0);

  float innerWrap = pow(clamp(tex.a - mix(blurAlphaIso, blurAlphaDir, 0.72), 0.0, 1.0), 0.74) * uLightWrap;
  vec3 rimSrgb = encodeSrgb(rimAcc * 0.62 + ambAcc * 0.25);
  vec3 wrapSource = max(bgWrapRgb * 1.14, clamp(rimSrgb, 0.0, 1.0));
  vec3 wrapAmt = clamp(wrapSource * innerWrap * 1.32, 0.0, 0.93);
  litSrgb = 1.0 - (1.0 - litSrgb) * (1.0 - wrapAmt);

  if (uCharOnly > 0.5) {
    // uCharOnly 是设计稿指标pass的开关（抽角色像素），场景路径恒传 0；
    // 纯背景走上面的零矩形早退，误传 1 会因 bgScene 被置零而整屏输出黑
    gl_FragColor = vec4(bgScene, 1.0);
    return;
  }

  float outerScatter = pow(clamp(blurAlphaIso * 1.42 - tex.a * 0.65, 0.0, 1.0), 1.15) * (1.0 - tex.a) * uLightWrap * 0.64;
  vec3 bgWithHalo = 1.0 - (1.0 - bgScene) * (1.0 - clamp(mix(wrapSource, vec3(1.0, 0.97, 0.92), 0.28) * outerScatter * 1.18, 0.0, 0.78));
  // 立绘曝光只乘在角色像素上，背景保持原样
  vec3 litExposed = clamp(litSrgb * uCharExposure, 0.0, 1.0);
  vec3 comp = mix(bgWithHalo, litExposed, tex.a);
  gl_FragColor = vec4(comp, 1.0);
}`;

/**
 * 分离式高斯模糊（9 tap）。光包裹需要一版模糊背景；泛光阶段会复用同一个
 * 程序，只是半径与迭代次数不同。
 */
export const BLUR_FRAGMENT_SHADER =
  COMMON_GLSL +
  `
uniform sampler2D uTex;
uniform vec2 uStep;
void main() {
  vec4 c = texture2D(uTex, vFboUv) * 0.22702703;
  vec2 o1 = uStep * 1.38461538;
  vec2 o2 = uStep * 3.23076923;
  c += texture2D(uTex, clamp(vFboUv + o1, 0.001, 0.999)) * 0.31621622;
  c += texture2D(uTex, clamp(vFboUv - o1, 0.001, 0.999)) * 0.31621622;
  c += texture2D(uTex, clamp(vFboUv + o2, 0.001, 0.999)) * 0.07027027;
  c += texture2D(uTex, clamp(vFboUv - o2, 0.001, 0.999)) * 0.07027027;
  gl_FragColor = c;
}`;
