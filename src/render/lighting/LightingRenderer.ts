/**
 * 背景光影渲染器（2.5D 多光源，第一层）。
 *
 * 职责边界：只负责「把一张背景图按多光源参数重新着色并画到 canvas」。
 * 不含合成层（泛光 / 景深）与角色材质，那些属于后续 PR；也不负责挂载到
 * 游戏舞台 —— 调用方给出舞台尺寸与背景图矩形即可。
 *
 * 与旧路径的关系：`GameBackground.vue` 的 CSS `filter` 与径向渐变叠加层
 * 继续服务旧存档。新路径只在场景带 `lights` 时启用，二者不叠加。
 */

import type { LightSource, LightingGlobalParams } from "../../api/services/scene";
import { BACKGROUND_FRAGMENT_SHADER, VERTEX_SHADER } from "./glsl";
import { packLights } from "./lights";

/** 舞台像素尺寸（已乘设备像素比）。 */
export interface StageSize {
  width: number;
  height: number;
}

/** 背景图在舞台中的像素矩形：`[x, y, width, height]`。 */
export type BackgroundRect = readonly [number, number, number, number];

/** 单帧渲染输入。 */
export interface BackgroundLightingInput {
  /** 全部灯；`enabled === false` 的会被跳过。 */
  lights: readonly LightSource[];
  /** 全局强度；缺省时取一组中性默认值（主强度 1，抬升 0）。 */
  global?: LightingGlobalParams | null;
  stage: StageSize;
  backgroundRect: BackgroundRect;
  /**
   * 背景平均色（sRGB，0-1）。用于暗部环境染色；不传则从背景贴图下采样估算。
   * 传入可以避免每帧重算，也方便测试固定输入。
   */
  averageColor?: readonly [number, number, number];
}

const NEUTRAL_GLOBAL: Required<Pick<LightingGlobalParams, "master" | "soft_knee" | "lift">> = {
  master: 1,
  soft_knee: 0.5,
  lift: 0,
};

/** 全屏四边形：两个三角形，裁剪空间坐标。 */
const QUAD_VERTICES = new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]);

function compileShader(gl: WebGLRenderingContext, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type);
  if (!shader) throw new Error("无法创建 shader 对象");
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(shader) ?? "(无日志)";
    gl.deleteShader(shader);
    const kind = type === gl.VERTEX_SHADER ? "vertex" : "fragment";
    throw new Error(`${kind} shader 编译失败: ${log}`);
  }
  return shader;
}

function linkProgram(
  gl: WebGLRenderingContext,
  vertexSrc: string,
  fragmentSrc: string,
): WebGLProgram {
  const program = gl.createProgram();
  if (!program) throw new Error("无法创建 program 对象");
  const vs = compileShader(gl, gl.VERTEX_SHADER, vertexSrc);
  const fs = compileShader(gl, gl.FRAGMENT_SHADER, fragmentSrc);
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
  return program;
}

/** 背景光影渲染器。一个实例对应一个 canvas。 */
export class BackgroundLightingRenderer {
  private readonly gl: WebGLRenderingContext;
  private readonly program: WebGLProgram;
  private readonly quadBuffer: WebGLBuffer;
  private readonly backgroundTexture: WebGLTexture;
  private readonly uniforms: Record<string, WebGLUniformLocation | null>;

  private backgroundLoaded = false;
  private averageColor: readonly [number, number, number] = [0.5, 0.5, 0.5];
  private disposed = false;

  static isSupported(): boolean {
    try {
      const probe = document.createElement("canvas");
      return Boolean(probe.getContext("webgl"));
    } catch {
      return false;
    }
  }

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

    this.program = linkProgram(gl, VERTEX_SHADER, BACKGROUND_FRAGMENT_SHADER);

    const buffer = gl.createBuffer();
    if (!buffer) throw new Error("无法创建顶点缓冲");
    this.quadBuffer = buffer;
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, QUAD_VERTICES, gl.STATIC_DRAW);

    const texture = gl.createTexture();
    if (!texture) throw new Error("无法创建背景纹理");
    this.backgroundTexture = texture;
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);

    const names = [
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
      "uExposure",
      "uLift",
      "uBgAvgSrgb",
    ];
    this.uniforms = {};
    for (const name of names) this.uniforms[name] = gl.getUniformLocation(this.program, name);
  }

  /** 设置画布像素尺寸。只在尺寸变化时重设，避免每帧清空缓冲。 */
  resize(width: number, height: number): boolean {
    const canvas = this.gl.canvas as HTMLCanvasElement;
    const w = Math.max(1, Math.round(width));
    const h = Math.max(1, Math.round(height));
    if (canvas.width === w && canvas.height === h) return false;
    canvas.width = w;
    canvas.height = h;
    return true;
  }

  /** 上传背景贴图，并顺带估算平均色（未显式传入 `averageColor` 时使用）。 */
  setBackground(source: TexImageSource): void {
    const gl = this.gl;
    gl.bindTexture(gl.TEXTURE_2D, this.backgroundTexture);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
    this.backgroundLoaded = true;
    this.averageColor = estimateAverageColor(source);
  }

  /** 清空背景，回到 1x1 空纹理。 */
  clearBackground(): void {
    const gl = this.gl;
    gl.bindTexture(gl.TEXTURE_2D, this.backgroundTexture);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    this.backgroundLoaded = false;
  }

  /** 画一帧。没有背景贴图时什么都不做。 */
  render(input: BackgroundLightingInput): void {
    if (this.disposed || !this.backgroundLoaded) return;
    const gl = this.gl;
    const u = this.uniforms;

    gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
    gl.useProgram(this.program);

    const { lights, stage, backgroundRect } = input;
    const master = input.global?.master ?? NEUTRAL_GLOBAL.master;
    const packed = packLights(lights, stage.width, stage.height, master);

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

    gl.uniform2f(u.uStageSize ?? null, stage.width, stage.height);
    gl.uniform4f(
      u.uBgRect ?? null,
      backgroundRect[0],
      backgroundRect[1],
      backgroundRect[2],
      backgroundRect[3],
    );
    gl.uniform1f(u.uSoftKnee ?? null, input.global?.soft_knee ?? NEUTRAL_GLOBAL.soft_knee);
    // 背景曝光默认 1；不传的话 uniform 保持 0，整幅背景会被乘成黑色
    gl.uniform1f(u.uExposure ?? null, input.global?.bg_exposure ?? 1);
    gl.uniform1f(u.uLift ?? null, input.global?.lift ?? NEUTRAL_GLOBAL.lift);

    const avg = input.averageColor ?? this.averageColor;
    gl.uniform3f(u.uBgAvgSrgb ?? null, avg[0], avg[1], avg[2]);

    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.backgroundTexture);
    gl.uniform1i(u.uBgTex ?? null, 0);

    gl.bindBuffer(gl.ARRAY_BUFFER, this.quadBuffer);
    const aPos = gl.getAttribLocation(this.program, "aPos");
    if (aPos >= 0) {
      gl.enableVertexAttribArray(aPos);
      gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);
    }
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  }

  dispose(): void {
    if (this.disposed) return;
    const gl = this.gl;
    gl.deleteProgram(this.program);
    gl.deleteBuffer(this.quadBuffer);
    gl.deleteTexture(this.backgroundTexture);
    this.disposed = true;
  }
}

/** 把贴图缩到 1x1 取平均色；失败时给中性灰，不让渲染中断。 */
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
