/**
 * 伪深度贴图生成。
 *
 * 2.5D 管线里的「深度」不是真几何深度，而是从立绘 alpha 估出来的近似值：
 * 越靠下越近（脚在画面下方、离观者更近），并给躯干一点横向鼓起，避免整块
 * 平面死板。着色器用它做自阴影的邻居比较（邻近像素更深就压暗），所以只需要
 * 相对关系正确，不需要精确。
 *
 * 生成规则与设计稿沙箱一致：
 * - alpha < 6 的像素置 0（背景不要参与自阴影）
 * - `baseDepth = (1 - y * 0.45) * 0.85 + bulge`，`bulge = max(0, 1 - x²) * 0.15`
 *
 * 输出是单通道信息塞进 RGB 的灰度图，与沙箱的 `uDepthTex` 取值口径一致。
 */

/** 立绘透明阈值：低于这个 alpha 视为背景。 */
const ALPHA_CUTOFF = 6;

/**
 * 从带 alpha 的立绘生成伪深度图。
 *
 * @param source 立绘（图片、canvas 或 ImageBitmap）
 * @param width 目标宽（像素）
 * @param height 目标高（像素）
 * @returns 灰度 canvas；调用方负责把它上传成纹理
 */
export function generatePseudoDepth(
  source: TexImageSource,
  width: number,
  height: number,
): HTMLCanvasElement {
  const w = Math.max(1, Math.round(width));
  const h = Math.max(1, Math.round(height));

  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return canvas;

  ctx.drawImage(source as CanvasImageSource, 0, 0, w, h);
  const imgData = ctx.getImageData(0, 0, w, h);
  const px = imgData.data;

  for (let y = 0; y < h; y++) {
    const normY = y / h;
    const row = y * w;
    for (let x = 0; x < w; x++) {
      const pIdx = (row + x) * 4;
      if (px[pIdx + 3] < ALPHA_CUTOFF) {
        px[pIdx] = 0;
        px[pIdx + 1] = 0;
        px[pIdx + 2] = 0;
        px[pIdx + 3] = 0;
        continue;
      }
      const normX = (x / w - 0.5) * 2;
      const bulge = Math.max(0, 1 - normX * normX) * 0.15;
      const baseDepth = (1 - normY * 0.45) * 0.85 + bulge;
      const dVal = Math.min(255, Math.max(0, Math.round(baseDepth * 255)));
      px[pIdx] = dVal;
      px[pIdx + 1] = dVal;
      px[pIdx + 2] = dVal;
      px[pIdx + 3] = 255;
    }
  }

  ctx.putImageData(imgData, 0, 0);
  return canvas;
}

/**
 * 平坦法线图：所有像素朝 +z。
 *
 * 没有上传真实法线图时用它兜底 —— 着色器会退回「只有 2D 线性渐变 + 轮廓光」的
 * 表现，不会因为缺法线而报错或全黑。
 */
export function createFlatNormalTexture(size = 1): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d");
  if (ctx) {
    ctx.fillStyle = "rgb(128, 128, 255)";
    ctx.fillRect(0, 0, size, size);
  }
  return canvas;
}

/** 读 TexImageSource 的像素尺寸（图片用 naturalWidth，canvas/bitmap 用 width）。 */
function readSourceSize(source: TexImageSource): { width: number; height: number } {
  const s = source as {
    naturalWidth?: number;
    naturalHeight?: number;
    width?: number;
    height?: number;
  };
  return {
    width: Math.max(1, Math.round(s.naturalWidth || s.width || 1)),
    height: Math.max(1, Math.round(s.naturalHeight || s.height || 1)),
  };
}

/** 立绘透明阈值，与 generatePseudoDepth 一致。 */
const NORMAL_ALPHA_CUTOFF = 6;

/**
 * 从立绘生成伪法线图。
 *
 * 两步叠加，与沙箱 `generatePseudoNormalMap` 逐字对应：
 * 1. **体积膨胀**：对 alpha 做两遍可分离盒状模糊，得到「剪影向内渗透的高程」。
 *    它让躯干、发簇、四肢呈现弧面，而不是一块斜平面。
 * 2. **细节微商**：对亮度做小步长中心差分，补上褶皱与发丝的方向。
 *
 * 输出是切线空间法线，编码 `rgb = n * 0.5 + 0.5`，与着色器 `rawN = rgb * 2 - 1` 对齐；
 * alpha 保留原图 alpha，所以剪影外的像素不会参与着色。
 *
 * @param source 立绘（图片 / canvas / ImageBitmap）
 * @param maxDim 长边上限，超过则等比缩小后再算（1024 足够，超过只增加耗时）
 * @param strength 凹凸强度，越大越「胖」
 */
export function generatePseudoNormal(
  source: TexImageSource,
  maxDim = 1024,
  strength = 2.5,
): HTMLCanvasElement {
  const srcSize = readSourceSize(source);
  const longest = Math.max(srcSize.width, srcSize.height);
  const scale = longest > maxDim ? maxDim / longest : 1;
  const cw = Math.max(64, Math.round(srcSize.width * scale));
  const ch = Math.max(64, Math.round(srcSize.height * scale));

  const canvas = document.createElement("canvas");
  canvas.width = cw;
  canvas.height = ch;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return canvas;

  ctx.drawImage(source as CanvasImageSource, 0, 0, cw, ch);
  const imgData = ctx.getImageData(0, 0, cw, ch);
  const px = imgData.data;
  const pixelCount = cw * ch;

  const luma = new Float32Array(pixelCount);
  const alpha = new Uint8Array(pixelCount);
  for (let i = 0; i < pixelCount; i++) {
    const p = i * 4;
    alpha[i] = px[p + 3];
    luma[i] = (0.2126 * px[p] + 0.7152 * px[p + 1] + 0.0722 * px[p + 2]) / 255;
  }

  // 1. alpha 两遍可分离盒状模糊 → 体积高程
  const blurAlpha = new Float32Array(pixelCount);
  const tempA = new Float32Array(pixelCount);
  const rVol = Math.max(4, Math.round(18 * scale));
  const win = 2 * rVol + 1;

  for (let y = 0; y < ch; y++) {
    const row = y * cw;
    let sum = 0;
    for (let x = 0; x < rVol; x++) sum += alpha[row + Math.min(x, cw - 1)];
    for (let x = 0; x < cw; x++) {
      if (x + rVol < cw) sum += alpha[row + x + rVol];
      if (x - rVol - 1 >= 0) sum -= alpha[row + x - rVol - 1];
      tempA[row + x] = sum / win;
    }
  }
  for (let x = 0; x < cw; x++) {
    let sum = 0;
    for (let y = 0; y < rVol; y++) sum += tempA[Math.min(y, ch - 1) * cw + x];
    for (let y = 0; y < ch; y++) {
      if (y + rVol < ch) sum += tempA[(y + rVol) * cw + x];
      if (y - rVol - 1 >= 0) sum -= tempA[(y - rVol - 1) * cw + x];
      blurAlpha[y * cw + x] = sum / (win * 255);
    }
  }

  const outData = ctx.createImageData(cw, ch);
  const out = outData.data;
  const stepVol = Math.max(2, Math.round(4 * scale));
  const stepDet = Math.max(1, Math.round(2 * scale));

  for (let y = 0; y < ch; y++) {
    const row = y * cw;
    const yVolU = Math.max(0, y - stepVol) * cw;
    const yVolD = Math.min(ch - 1, y + stepVol) * cw;
    const yDetU = Math.max(0, y - stepDet) * cw;
    const yDetD = Math.min(ch - 1, y + stepDet) * cw;

    for (let x = 0; x < cw; x++) {
      const idx = row + x;
      const pIdx = idx * 4;
      const a = alpha[idx];

      if (a < NORMAL_ALPHA_CUTOFF) {
        out[pIdx] = 128;
        out[pIdx + 1] = 128;
        out[pIdx + 2] = 255;
        out[pIdx + 3] = 0;
        continue;
      }

      const xVolL = Math.max(0, x - stepVol);
      const xVolR = Math.min(cw - 1, x + stepVol);
      const xDetL = Math.max(0, x - stepDet);
      const xDetR = Math.min(cw - 1, x + stepDet);

      const dVolX = (blurAlpha[row + xVolR] - blurAlpha[row + xVolL]) / (xVolR - xVolL || 1);
      const dVolY = (blurAlpha[yVolD + x] - blurAlpha[yVolU + x]) / (2 * stepVol);
      const dDetX = (luma[row + xDetR] - luma[row + xDetL]) / (xDetR - xDetL || 1);
      const dDetY = (luma[yDetD + x] - luma[yDetU + x]) / (2 * stepDet);

      let nx = -(dVolX * 2.8 + dDetX * 1.8) * strength;
      let ny = (dVolY * 2.8 + dDetY * 1.8) * strength;
      let nz = 1;
      const invLen = 1 / Math.hypot(nx, ny, nz);
      nx *= invLen;
      ny *= invLen;
      nz *= invLen;

      out[pIdx] = Math.round((nx * 0.5 + 0.5) * 255);
      out[pIdx + 1] = Math.round((ny * 0.5 + 0.5) * 255);
      out[pIdx + 2] = Math.round((nz * 0.5 + 0.5) * 255);
      out[pIdx + 3] = a;
    }
  }

  ctx.putImageData(outData, 0, 0);
  return canvas;
}
