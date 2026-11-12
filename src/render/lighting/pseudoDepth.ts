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
 * 没有上传真实法线图时用它兜底 —— 着色器会退回「只有 2D 线性渐变 + 轮廓光」
 * 的表现，不会因为缺法线而报错或全黑。
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
