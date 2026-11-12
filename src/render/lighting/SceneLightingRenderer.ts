/**
 * 场景光影渲染器（2.5D 多光源，第二层：角色合成）。
 *
 * 一条完整的绘制链：
 *
 * ```
 * 背景 pass        -> FBO(bg)          逐像素多光源着色
 * 模糊 pass ×2 pass -> FBO(wrap)       给光包裹用的柔化背景（横向 + 纵向）
 * 角色合成 pass    -> 屏幕              立绘 × 灯光 × 伪法线/深度 × 光包裹
 * ```
 *
 * 与 `BackgroundLightingRenderer` 的关系：那个类只做第一层、直接画到屏幕，
 * 适合没有角色的场景（或单独调试背景光照）。这个类是完整版，背景先落到离屏
 * 目标再与立绘合成。两者共用同一套 GLSL 与灯光打包逻辑，数值口径一致。
 *
 * 不含泛光、景深与指标统计 —— 那些属于「合成与指标」层。
 */

import type {
  LightSource,
  LightingCameraParams,
  LightingCompositingParams,
  LightingGlobalParams,
  LightingMaterialParams,
} from "../../api/services/scene";
import { BLUR_FRAGMENT_SHADER, CHARACTER_FRAGMENT_SHADER } from "./characterShader";
import { BACKGROUND_FRAGMENT_SHADER, VERTEX_SHADER } from "./glsl";
import { BLOOM_PREFILTER_FRAGMENT_SHADER, FINAL_COMPOSITE_FRAGMENT_SHADER } from "./postProcess";
import { packLights } from "./lights";
import { createFlatNormalTexture, generatePseudoDepth } from "./pseudoDepth";

/** 舞台像素尺寸（已乘设备像素比）。 */
export interface StageSize {
  width: number;
  height: number;
}

/** 矩形：`[x, y, width, height]`，单位为舞台像素。 */
export type Rect = readonly [number, number, number, number];

/** 单帧渲染输入。 */
export interface SceneLightingFrame {
  stage: StageSize;
  /** 背景图在舞台上的覆盖矩形（cover 缩放后可能超出舞台，允许负值）。 */
  backgroundRect: Rect;
  /** 立绘在舞台上的矩形。 */
  characterRect: Rect;
  lights: readonly LightSource[];
  global?: LightingGlobalParams | null;
  compositing?: LightingCompositingParams | null;
  material?: LightingMaterialParams | null;
  /** 相机参数；目前只有 `dof` 参与渲染（背景景深）。 */
  camera?: LightingCameraParams | null;
  /** 背景平均色（sRGB 0-1）；不传则用上传背景时算好的值。 */
  averageColor?: readonly [number, number, number];
  /**
   * overlay 径向渐变异色层（旧界面的双色光晕 + mix-blend-mode）。
   * 这组字段一直在 `LightingParams` 里，2.5D 路径此前没有消费。
   */
  overlay?: SceneOverlayParams | null;
}

/** overlay 径向渐变的渲染参数。 */
export interface SceneOverlayParams {
  enabled: boolean;
  /** 中心色（hex） */
  color1: string;
  /** 边缘色（hex） */
  color2: string;
  /** 半径百分比，0–100 */
  radius: number;
  /** 不透明度，0–1 */
  opacity: number;
  /** 中心点，舞台归一化坐标（0–1，y 向下） */
  center: readonly [number, number];
  /** CSS mix-blend-mode 名称 */
  mode: string;
}

/** 立绘可选贴图。`depth` 传 `"auto"` 时从 alpha 生成。 */
export interface CharacterMaps {
  normal?: TexImageSource | null;
  depth?: TexImageSource | "auto" | null;
}

/** CSS mix-blend-mode → 着色器里的模式编码，见 postProcess.ts 的 blendMode()。 */
const BLEND_MODE_CODE: Record<string, number> = {
  normal: 0,
  multiply: 1,
  screen: 2,
  overlay: 3,
  "soft-light": 4,
  darken: 5,
  lighten: 6,
};

function blendModeCode(mode: string | null | undefined): number {
  if (!mode) return 0;
  return BLEND_MODE_CODE[mode.trim().toLowerCase()] ?? 0;
}

/** hex → sRGB 0-1。调色在 sRGB 空间做，和 CSS filter 的口径一致。 */
function hexToSrgb(hex: string | null | undefined): [number, number, number] {
  const m = /^#([0-9a-fA-F]{6})$/.exec((hex ?? "").trim());
  if (!m) return [1, 1, 1];
  const v = m[1];
  return [
    parseInt(v.slice(0, 2), 16) / 255,
    parseInt(v.slice(2, 4), 16) / 255,
    parseInt(v.slice(4, 6), 16) / 255,
  ];
}

/** 色相角 → 最大分量为 1 的 RGB，用作乘性偏色（不让整体变亮或变暗）。 */
function hueToUnitRgb(deg: number): [number, number, number] {
  const h = ((((deg % 360) + 360) % 360) / 60) % 6;
  const x = 1 - Math.abs((h % 2) - 1);
  if (h < 1) return [1, x, 0];
  if (h < 2) return [x, 1, 0];
  if (h < 3) return [0, 1, x];
  if (h < 4) return [0, x, 1];
  if (h < 5) return [x, 0, 1];
  return [1, 0, x];
}

function clamp01(v: number): number {
  return Math.min(1, Math.max(0, v));
}

interface RenderTarget {
  fbo: WebGLFramebuffer;
  tex: WebGLTexture;
  width: number;
  height: number;
}

interface CompiledProgram {
  program: WebGLProgram;
  uniforms: Record<string, WebGLUniformLocation | null>;
}

const QUAD_VERTICES = new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]);

/** 光包裹用的模糊：半径与迭代次数取自设计稿，改动前先在沙箱里对比。 */
const WRAP_BLUR_RADIUS = 1.45;
const WRAP_BLUR_ITERATIONS = 2;

const DEFAULT_GLOBAL: LightingGlobalParams = {
  master: 1,
  ambient_blend: 0.5,
  albedo_protect: 0.5,
  soft_knee: 0.5,
  lift: 0,
  mono: 0,
  // 调色组默认中性，与旧存档行为一致
  saturation: 1,
  contrast: 1,
  tint_hue: 0,
  tint_amount: 0,
  char_exposure: 1,
  bg_exposure: 1,
};

const DEFAULT_COMPOSITING: LightingCompositingParams = {
  light_wrap: 0.6,
  rim_width: 6,
  bloom_int: 0,
  bloom_thresh: 0.7,
  glow_color: "#ffffff",
};

const DEFAULT_MATERIAL: LightingMaterialParams = {
  use_normal: true,
  normal_strength: 1,
  cel_thresh: 0.5,
  cel_softness: 0.06,
  spec_int: 0.45,
  spec_gloss: 24,
  self_shadow_int: 0.35,
  use_depth: true,
};

const BG_UNIFORMS = [
  "uLCount",
  "uKeyIndex",
  "uLType[0]",
  "uLColor[0]",
  "uLInt[0]",
  "uLPos[0]",
  "uLRad[0]",
  "uLSoft[0]",
  "uLAxis[0]",
  "uLCos[0]",
  "uLPen[0]",
  "uBgTex",
  "uStageSize",
  "uBgRect",
  "uSoftKnee",
  "uLift",
  "uBgAvgSrgb",
  "uExposure",
];

const BLUR_UNIFORMS = ["uTex", "uStep"];

const BLOOM_UNIFORMS = ["uSceneTex", "uThresh", "uGlowColor"];

const FINAL_UNIFORMS = [
  "uSceneTex",
  "uBloom1Tex",
  "uBloom2Tex",
  "uBloomInt",
  "uMono",
  "uSaturation",
  "uContrast",
  "uTintColor",
  "uTintAmount",
  "uOverlayEnabled",
  "uOverlayColor1",
  "uOverlayColor2",
  "uOverlayCenter",
  "uOverlayRadius",
  "uOverlayMode",
  "uStageAspect",
];

const CHARACTER_UNIFORMS = [
  "uLCount",
  "uKeyIndex",
  "uLType[0]",
  "uLColor[0]",
  "uLInt[0]",
  "uLPos[0]",
  "uLRad[0]",
  "uLSoft[0]",
  "uLAxis[0]",
  "uLCos[0]",
  "uLPen[0]",
  "uCharTex",
  "uBgSceneTex",
  "uBgWrapTex",
  "uNormalTex",
  "uDepthTex",
  "uHasNormal",
  "uHasDepth",
  "uNormalStrength",
  "uCelThresh",
  "uCelSoftness",
  "uSpecInt",
  "uSpecGloss",
  "uSelfShadowInt",
  "uStageSize",
  "uCharRect",
  "uAlbedoProtect",
  "uSoftKnee",
  "uLift",
  "uAmbientBlend",
  "uLightWrap",
  "uRimWidth",
  "uCharOnly",
  "uBgAvgSrgb",
  "uCharExposure",
];

function compileShader(gl: WebGLRenderingContext, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type);
  if (!shader) throw new Error("无法创建 shader 对象");
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(shader) ?? "(无日志)";
    gl.deleteShader(shader);
    throw new Error(`${type === gl.VERTEX_SHADER ? "vertex" : "fragment"} shader 编译失败: ${log}`);
  }
  return shader;
}

function linkProgram(
  gl: WebGLRenderingContext,
  fragmentSource: string,
  uniformNames: string[],
): CompiledProgram {
  const program = gl.createProgram();
  if (!program) throw new Error("无法创建 program 对象");
  const vs = compileShader(gl, gl.VERTEX_SHADER, VERTEX_SHADER);
  const fs = compileShader(gl, gl.FRAGMENT_SHADER, fragmentSource);
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);
  gl.deleteShader(vs);
  gl.deleteShader(fs);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    const log = gl.getProgramInfoLog(program) ?? "(无日志)";
    gl.deleteProgram(program);
    throw new Error(`program 链接失败: ${log}`);
  }
  const uniforms: Record<string, WebGLUniformLocation | null> = {};
  for (const name of uniformNames) uniforms[name] = gl.getUniformLocation(program, name);
  return { program, uniforms };
}

function createTarget(gl: WebGLRenderingContext, width: number, height: number): RenderTarget {
  const tex = gl.createTexture();
  const fbo = gl.createFramebuffer();
  if (!tex || !fbo) throw new Error("无法创建离屏渲染目标");
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
  gl.bindFramebuffer(gl.FRAMEBUFFER, fbo);
  gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  return { fbo, tex, width, height };
}

/** 完整场景光影渲染器（背景 + 立绘合成）。 */
export class SceneLightingRenderer {
  private readonly gl: WebGLRenderingContext;
  private readonly bgProgram: CompiledProgram;
  private readonly blurProgram: CompiledProgram;
  private readonly charProgram: CompiledProgram;
  private readonly bloomProgram: CompiledProgram;
  private readonly finalProgram: CompiledProgram;
  private readonly quadBuffer: WebGLBuffer;

  private readonly backgroundTexture: WebGLTexture;
  private readonly characterTexture: WebGLTexture;
  private readonly normalTexture: WebGLTexture;
  private readonly depthTexture: WebGLTexture;

  private bgTarget: RenderTarget | null = null;
  private sceneTarget: RenderTarget | null = null;
  private wrapTargetA: RenderTarget | null = null;
  private wrapTargetB: RenderTarget | null = null;
  private dofTargetA: RenderTarget | null = null;
  private dofTargetB: RenderTarget | null = null;
  private bloomTarget1A: RenderTarget | null = null;
  private bloomTarget1B: RenderTarget | null = null;
  private bloomTarget2A: RenderTarget | null = null;
  private bloomTarget2B: RenderTarget | null = null;

  private backgroundReady = false;
  private characterReady = false;
  private normalReady = false;
  private depthReady = false;
  private averageColor: readonly [number, number, number] = [0.5, 0.5, 0.5];
  private disposed = false;

  constructor(canvas: HTMLCanvasElement) {
    const gl = canvas.getContext("webgl", {
      alpha: false,
      antialias: false,
      depth: false,
      premultipliedAlpha: false,
      preserveDrawingBuffer: true,
    });
    if (!gl) throw new Error("当前环境不支持 WebGL");
    this.gl = gl;

    this.bgProgram = linkProgram(gl, BACKGROUND_FRAGMENT_SHADER, BG_UNIFORMS);
    this.blurProgram = linkProgram(gl, BLUR_FRAGMENT_SHADER, BLUR_UNIFORMS);
    this.charProgram = linkProgram(gl, CHARACTER_FRAGMENT_SHADER, CHARACTER_UNIFORMS);
    this.bloomProgram = linkProgram(gl, BLOOM_PREFILTER_FRAGMENT_SHADER, BLOOM_UNIFORMS);
    this.finalProgram = linkProgram(gl, FINAL_COMPOSITE_FRAGMENT_SHADER, FINAL_UNIFORMS);

    const buffer = gl.createBuffer();
    if (!buffer) throw new Error("无法创建顶点缓冲");
    this.quadBuffer = buffer;
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, QUAD_VERTICES, gl.STATIC_DRAW);

    this.backgroundTexture = this.createTexture();
    this.characterTexture = this.createTexture();
    this.normalTexture = this.createTexture();
    this.depthTexture = this.createTexture();
    this.uploadTexture(this.normalTexture, createFlatNormalTexture());
  }

  static isSupported(): boolean {
    try {
      return Boolean(document.createElement("canvas").getContext("webgl"));
    } catch {
      return false;
    }
  }

  private createTexture(): WebGLTexture {
    const gl = this.gl;
    const tex = gl.createTexture();
    if (!tex) throw new Error("无法创建纹理");
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    return tex;
  }

  private uploadTexture(tex: WebGLTexture, source: TexImageSource): void {
    const gl = this.gl;
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
  }

  resize(width: number, height: number): void {
    const gl = this.gl;
    const canvas = gl.canvas as HTMLCanvasElement;
    const w = Math.max(1, Math.round(width));
    const h = Math.max(1, Math.round(height));
    if (canvas.width === w && canvas.height === h && this.bgTarget) return;
    canvas.width = w;
    canvas.height = h;
    const allTargets = [
      this.bgTarget,
      this.sceneTarget,
      this.wrapTargetA,
      this.wrapTargetB,
      this.dofTargetA,
      this.dofTargetB,
      this.bloomTarget1A,
      this.bloomTarget1B,
      this.bloomTarget2A,
      this.bloomTarget2B,
    ];
    for (const target of allTargets) {
      if (target) {
        gl.deleteFramebuffer(target.fbo);
        gl.deleteTexture(target.tex);
      }
    }

    // 分层尺寸与设计稿一致：全分辨率场景、半分辨率泛光/景深、四分之一光包裹
    const hw = Math.max(1, w >> 1);
    const hh = Math.max(1, h >> 1);
    const qw = Math.max(1, w >> 2);
    const qh = Math.max(1, h >> 2);

    this.bgTarget = createTarget(gl, w, h);
    this.sceneTarget = createTarget(gl, w, h);
    this.dofTargetA = createTarget(gl, hw, hh);
    this.dofTargetB = createTarget(gl, hw, hh);
    this.bloomTarget1A = createTarget(gl, hw, hh);
    this.bloomTarget1B = createTarget(gl, hw, hh);
    this.wrapTargetA = createTarget(gl, qw, qh);
    this.wrapTargetB = createTarget(gl, qw, qh);
    this.bloomTarget2A = createTarget(gl, qw, qh);
    this.bloomTarget2B = createTarget(gl, qw, qh);
  }

  setBackground(source: TexImageSource): void {
    this.uploadTexture(this.backgroundTexture, source);
    this.backgroundReady = true;
    this.averageColor = estimateAverageColor(source);
  }

  setCharacter(source: TexImageSource, maps: CharacterMaps = {}): void {
    this.uploadTexture(this.characterTexture, source);
    this.characterReady = true;

    if (maps.normal) {
      this.uploadTexture(this.normalTexture, maps.normal);
      this.normalReady = true;
    } else {
      this.uploadTexture(this.normalTexture, createFlatNormalTexture());
      this.normalReady = false;
    }

    const depthSource = maps.depth ?? "auto";
    if (depthSource === "auto") {
      const size = readSourceSize(source);
      this.uploadTexture(this.depthTexture, generatePseudoDepth(source, size.width, size.height));
      this.depthReady = true;
    } else if (depthSource) {
      this.uploadTexture(this.depthTexture, depthSource);
      this.depthReady = true;
    } else {
      this.uploadTexture(this.depthTexture, createFlatNormalTexture());
      this.depthReady = false;
    }
  }

  clearCharacter(): void {
    this.characterReady = false;
  }

  /** 画一帧。背景未就绪时什么都不做。 */
  render(frame: SceneLightingFrame): void {
    if (this.disposed || !this.backgroundReady) return;
    const gl = this.gl;
    const bgTarget = this.bgTarget;
    const wrapA = this.wrapTargetA;
    const wrapB = this.wrapTargetB;
    if (!bgTarget || !wrapA || !wrapB) return;

    const { stage, lights } = frame;
    const globalParams = { ...DEFAULT_GLOBAL, ...(frame.global ?? {}) };
    const compositing = { ...DEFAULT_COMPOSITING, ...(frame.compositing ?? {}) };
    const material = { ...DEFAULT_MATERIAL, ...(frame.material ?? {}) };
    const packed = packLights(lights, stage.width, stage.height, globalParams.master);

    // ---- Pass 1：背景多光源着色 -> bgTarget ----
    gl.bindFramebuffer(gl.FRAMEBUFFER, bgTarget.fbo);
    gl.viewport(0, 0, bgTarget.width, bgTarget.height);
    gl.useProgram(this.bgProgram.program);
    this.applyLightUniforms(this.bgProgram, packed);
    gl.uniform2f(this.bgProgram.uniforms.uStageSize ?? null, stage.width, stage.height);
    gl.uniform4f(
      this.bgProgram.uniforms.uBgRect ?? null,
      frame.backgroundRect[0],
      frame.backgroundRect[1],
      frame.backgroundRect[2],
      frame.backgroundRect[3],
    );
    gl.uniform1f(this.bgProgram.uniforms.uSoftKnee ?? null, globalParams.soft_knee);
    gl.uniform1f(this.bgProgram.uniforms.uLift ?? null, globalParams.lift);
    gl.uniform1f(this.bgProgram.uniforms.uExposure ?? null, globalParams.bg_exposure ?? 1);
    const avg = frame.averageColor ?? this.averageColor;
    gl.uniform3f(this.bgProgram.uniforms.uBgAvgSrgb ?? null, avg[0], avg[1], avg[2]);
    this.bindTextureUnit(0, this.backgroundTexture);
    gl.uniform1i(this.bgProgram.uniforms.uBgTex ?? null, 0);
    this.drawQuad();

    // ---- Pass 2：双向高斯模糊 -> wrapB（光包裹用）----
    this.blur(bgTarget.tex, wrapA, wrapB, WRAP_BLUR_RADIUS, WRAP_BLUR_ITERATIONS);

    // ---- Pass 3：背景景深（可选）----
    let bgSceneTex: WebGLTexture = bgTarget.tex;
    const dof = frame.camera?.dof ?? 0;
    const dofA = this.dofTargetA;
    const dofB = this.dofTargetB;
    if (dof > 0.05 && dofA && dofB) {
      const dofScale = Math.max(0.25, (dof / 6) * (Math.min(dofB.width, dofB.height) / 360));
      this.blur(bgTarget.tex, dofA, dofB, dofScale, 2);
      bgSceneTex = dofB.tex;
    }

    // ---- Pass 4：角色合成 -> sceneTarget ----
    const sceneTarget = this.sceneTarget;
    if (!sceneTarget) return;
    gl.bindFramebuffer(gl.FRAMEBUFFER, sceneTarget.fbo);
    gl.viewport(0, 0, sceneTarget.width, sceneTarget.height);
    gl.useProgram(this.charProgram.program);
    this.applyLightUniforms(this.charProgram, packed);

    this.bindTextureUnit(0, this.characterTexture);
    this.bindTextureUnit(1, bgSceneTex);
    this.bindTextureUnit(2, wrapB.tex);
    this.bindTextureUnit(3, this.normalTexture);
    this.bindTextureUnit(4, this.depthTexture);
    const cu = this.charProgram.uniforms;
    gl.uniform1i(cu.uCharTex ?? null, 0);
    gl.uniform1i(cu.uBgSceneTex ?? null, 1);
    gl.uniform1i(cu.uBgWrapTex ?? null, 2);
    gl.uniform1i(cu.uNormalTex ?? null, 3);
    gl.uniform1i(cu.uDepthTex ?? null, 4);

    gl.uniform1f(cu.uHasNormal ?? null, this.normalReady && material.use_normal ? 1 : 0);
    gl.uniform1f(cu.uHasDepth ?? null, this.depthReady && material.use_depth ? 1 : 0);
    gl.uniform1f(cu.uNormalStrength ?? null, material.normal_strength);
    gl.uniform1f(cu.uCelThresh ?? null, material.cel_thresh);
    gl.uniform1f(cu.uCelSoftness ?? null, material.cel_softness);
    gl.uniform1f(cu.uSpecInt ?? null, material.spec_int);
    gl.uniform1f(cu.uSpecGloss ?? null, material.spec_gloss);
    gl.uniform1f(cu.uSelfShadowInt ?? null, material.self_shadow_int);
    gl.uniform2f(cu.uStageSize ?? null, stage.width, stage.height);
    gl.uniform4f(
      cu.uCharRect ?? null,
      frame.characterRect[0],
      frame.characterRect[1],
      frame.characterRect[2],
      frame.characterRect[3],
    );
    gl.uniform1f(cu.uAlbedoProtect ?? null, globalParams.albedo_protect);
    gl.uniform1f(cu.uSoftKnee ?? null, globalParams.soft_knee);
    gl.uniform1f(cu.uLift ?? null, globalParams.lift);
    gl.uniform1f(cu.uAmbientBlend ?? null, globalParams.ambient_blend);
    gl.uniform1f(cu.uCharExposure ?? null, globalParams.char_exposure ?? 1);
    gl.uniform1f(cu.uLightWrap ?? null, compositing.light_wrap);
    gl.uniform1f(cu.uRimWidth ?? null, compositing.rim_width);
    gl.uniform1f(cu.uCharOnly ?? null, this.characterReady ? 0 : 1);
    gl.uniform3f(cu.uBgAvgSrgb ?? null, avg[0], avg[1], avg[2]);
    this.drawQuad();

    // ---- Pass 5：亮部提取 + 两级泛光 ----
    let bloom1Tex: WebGLTexture | null = null;
    let bloom2Tex: WebGLTexture | null = null;
    const b1a = this.bloomTarget1A;
    const b1b = this.bloomTarget1B;
    const b2a = this.bloomTarget2A;
    const b2b = this.bloomTarget2B;
    if (compositing.bloom_int > 0.005 && b1a && b1b && b2a && b2b && dofA) {
      gl.bindFramebuffer(gl.FRAMEBUFFER, b1a.fbo);
      gl.viewport(0, 0, b1a.width, b1a.height);
      gl.useProgram(this.bloomProgram.program);
      this.bindTextureUnit(0, sceneTarget.tex);
      gl.uniform1i(this.bloomProgram.uniforms.uSceneTex ?? null, 0);
      gl.uniform1f(this.bloomProgram.uniforms.uThresh ?? null, compositing.bloom_thresh);
      const glow = hexToSrgb(compositing.glow_color);
      gl.uniform3f(this.bloomProgram.uniforms.uGlowColor ?? null, glow[0], glow[1], glow[2]);
      this.drawQuad();

      // 第一级：小半径，给高光描边；结果留在 b1b
      this.blur(b1a.tex, dofA, b1b, 1.25, 2);
      // 第二级：大半径，铺成雾状光晕；结果留在 b2b
      this.blur(b1b.tex, b2a, b2b, 2.25, 3);
      bloom1Tex = b1b.tex;
      bloom2Tex = b2b.tex;
    }

    // ---- Pass 6：最终合成 -> 屏幕 ----
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
    gl.useProgram(this.finalProgram.program);
    const fu = this.finalProgram.uniforms;
    this.bindTextureUnit(0, sceneTarget.tex);
    this.bindTextureUnit(1, bloom1Tex ?? sceneTarget.tex);
    this.bindTextureUnit(2, bloom2Tex ?? sceneTarget.tex);
    gl.uniform1i(fu.uSceneTex ?? null, 0);
    gl.uniform1i(fu.uBloom1Tex ?? null, 1);
    gl.uniform1i(fu.uBloom2Tex ?? null, 2);
    gl.uniform1f(fu.uBloomInt ?? null, bloom1Tex ? compositing.bloom_int : 0);
    gl.uniform1f(fu.uMono ?? null, globalParams.mono);

    // 调色：与旧 CSS filter 的 saturation / contrast / sepia 对应，但作用于整幅合成结果
    gl.uniform1f(fu.uSaturation ?? null, globalParams.saturation ?? 1);
    gl.uniform1f(fu.uContrast ?? null, globalParams.contrast ?? 1);
    const tint = hueToUnitRgb(globalParams.tint_hue ?? 0);
    gl.uniform3f(fu.uTintColor ?? null, tint[0], tint[1], tint[2]);
    gl.uniform1f(fu.uTintAmount ?? null, clamp01(globalParams.tint_amount ?? 0));

    // overlay：中心点来自旧 light_x / light_y（0–100 → 0–1）
    const ov = frame.overlay;
    const ovOn = !!ov?.enabled;
    gl.uniform1f(fu.uOverlayEnabled ?? null, ovOn ? 1 : 0);
    gl.uniform1f(fu.uStageAspect ?? null, stage.height > 0 ? stage.width / stage.height : 1);
    if (ov && ovOn) {
      const c1 = hexToSrgb(ov.color1);
      const c2 = hexToSrgb(ov.color2);
      gl.uniform4f(fu.uOverlayColor1 ?? null, c1[0], c1[1], c1[2], clamp01(ov.opacity));
      gl.uniform4f(fu.uOverlayColor2 ?? null, c2[0], c2[1], c2[2], 1);
      gl.uniform2f(fu.uOverlayCenter ?? null, ov.center[0], ov.center[1]);
      gl.uniform1f(fu.uOverlayRadius ?? null, Math.max(0.02, (ov.radius ?? 50) / 100));
      gl.uniform1f(fu.uOverlayMode ?? null, blendModeCode(ov.mode));
    }
    this.drawQuad();
  }

  private applyLightUniforms(target: CompiledProgram, packed: ReturnType<typeof packLights>): void {
    const gl = this.gl;
    const u = target.uniforms;
    gl.uniform1i(u.uLCount ?? null, packed.count);
    gl.uniform1i(u.uKeyIndex ?? null, packed.keyIndex);
    gl.uniform1iv(u["uLType[0]"] ?? null, packed.types);
    gl.uniform3fv(u["uLColor[0]"] ?? null, packed.colors);
    gl.uniform1fv(u["uLInt[0]"] ?? null, packed.intens);
    gl.uniform3fv(u["uLPos[0]"] ?? null, packed.pos);
    gl.uniform1fv(u["uLRad[0]"] ?? null, packed.rads);
    gl.uniform1fv(u["uLSoft[0]"] ?? null, packed.softs);
    gl.uniform3fv(u["uLAxis[0]"] ?? null, packed.axes);
    gl.uniform1fv(u["uLCos[0]"] ?? null, packed.cos);
    gl.uniform1fv(u["uLPen[0]"] ?? null, packed.pen);
  }

  private blur(
    source: WebGLTexture,
    targetA: RenderTarget,
    targetB: RenderTarget,
    radiusScale: number,
    iterations: number,
  ): void {
    const gl = this.gl;
    const u = this.blurProgram.uniforms;
    gl.useProgram(this.blurProgram.program);
    gl.uniform1i(u.uTex ?? null, 0);

    let current = source;
    for (let i = 0; i < iterations; i++) {
      gl.bindFramebuffer(gl.FRAMEBUFFER, targetA.fbo);
      gl.viewport(0, 0, targetA.width, targetA.height);
      this.bindTextureUnit(0, current);
      gl.uniform2f(u.uStep ?? null, radiusScale / targetA.width, 0);
      this.drawQuad();

      gl.bindFramebuffer(gl.FRAMEBUFFER, targetB.fbo);
      gl.viewport(0, 0, targetB.width, targetB.height);
      this.bindTextureUnit(0, targetA.tex);
      gl.uniform2f(u.uStep ?? null, 0, radiusScale / targetB.height);
      this.drawQuad();

      current = targetB.tex;
    }
  }

  private bindTextureUnit(unit: number, tex: WebGLTexture): void {
    const gl = this.gl;
    gl.activeTexture(gl.TEXTURE0 + unit);
    gl.bindTexture(gl.TEXTURE_2D, tex);
  }

  private drawQuad(): void {
    const gl = this.gl;
    gl.bindBuffer(gl.ARRAY_BUFFER, this.quadBuffer);
    const aPos = gl.getAttribLocation(gl.getParameter(gl.CURRENT_PROGRAM) as WebGLProgram, "aPos");
    if (aPos >= 0) {
      gl.enableVertexAttribArray(aPos);
      gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);
    }
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  }

  dispose(): void {
    if (this.disposed) return;
    const gl = this.gl;
    const allTargets = [
      this.bgTarget,
      this.sceneTarget,
      this.wrapTargetA,
      this.wrapTargetB,
      this.dofTargetA,
      this.dofTargetB,
      this.bloomTarget1A,
      this.bloomTarget1B,
      this.bloomTarget2A,
      this.bloomTarget2B,
    ];
    for (const target of allTargets) {
      if (target) {
        gl.deleteFramebuffer(target.fbo);
        gl.deleteTexture(target.tex);
      }
    }
    gl.deleteProgram(this.bgProgram.program);
    gl.deleteProgram(this.blurProgram.program);
    gl.deleteProgram(this.charProgram.program);
    gl.deleteProgram(this.bloomProgram.program);
    gl.deleteProgram(this.finalProgram.program);
    gl.deleteBuffer(this.quadBuffer);
    for (const tex of [
      this.backgroundTexture,
      this.characterTexture,
      this.normalTexture,
      this.depthTexture,
    ]) {
      gl.deleteTexture(tex);
    }
    this.disposed = true;
  }
}

function readSourceSize(source: TexImageSource): { width: number; height: number } {
  const anySource = source as {
    width?: number;
    height?: number;
    videoWidth?: number;
    videoHeight?: number;
  };
  return {
    width: anySource.width ?? anySource.videoWidth ?? 1,
    height: anySource.height ?? anySource.videoHeight ?? 1,
  };
}

function estimateAverageColor(source: TexImageSource): [number, number, number] {
  try {
    const canvas = document.createElement("canvas");
    canvas.width = 1;
    canvas.height = 1;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) return [0.5, 0.5, 0.5];
    ctx.drawImage(source as CanvasImageSource, 0, 0, 1, 1);
    const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data;
    return [r / 255, g / 255, b / 255];
  } catch {
    return [0.5, 0.5, 0.5];
  }
}
