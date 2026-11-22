/**
 * WebGL 贴图用的图片加载。
 *
 * 三条路依次试，拿到「干净位图」就返回：
 * 1. 先按 CORS 方式加载（crossOrigin="anonymous"）。Tauri 的 asset 协议带 ACAO，
 *    这样拿到的图是干净的，WebGL 能上传、也能读像素生成法线/深度。
 * 2. 失败再走 fetch 取字节、createImageBitmap 解码：位图天然不受跨源限制。
 * 3. 两条都不行才普通加载。这种图能显示，但可能污染画布 —— 污染图喂给
 *    texImage2D 会直接抛 SecurityError，等于整块画布全黑，所以这里探一次并
 *    标出 tainted，由调用方决定弃用。
 *
 * 工作台与聊天舞台必须共用这一套：聊天侧以前只有第 1 条路，加载失败就整场
 * 退回 CSS 光影，而工作台照样出图，表现出来就是「工作台调了、游戏里没反应」。
 */

export interface LoadedImage {
  source: TexImageSource;
  width: number;
  height: number;
  /** true = 画布会被污染，WebGL 不能上传 */
  tainted: boolean;
}

function loadImg(src: string, anonymous: boolean): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const img = new Image();
    if (anonymous) img.crossOrigin = "anonymous";
    img.onload = () => resolve(img.naturalWidth ? img : null);
    img.onerror = () => resolve(null);
    img.src = src;
  });
}

/** 画一张 1×1 再读回来，能读就是干净图 */
function isReadable(source: TexImageSource): boolean {
  try {
    const c = document.createElement("canvas");
    c.width = 1;
    c.height = 1;
    const ctx = c.getContext("2d", { willReadFrequently: true });
    if (!ctx) return false;
    ctx.drawImage(source as CanvasImageSource, 0, 0, 1, 1);
    ctx.getImageData(0, 0, 1, 1);
    return true;
  } catch {
    return false;
  }
}

export async function loadTexImage(src: string): Promise<LoadedImage | null> {
  const anon = await loadImg(src, true);
  if (anon) {
    return { source: anon, width: anon.naturalWidth, height: anon.naturalHeight, tainted: false };
  }
  try {
    const res = await fetch(src);
    if (res.ok) {
      const bmp = await createImageBitmap(await res.blob());
      return { source: bmp, width: bmp.width, height: bmp.height, tainted: false };
    }
  } catch {
    /* 跨源且无 ACAO 时 fetch 也会失败，继续往下 */
  }
  const plain = await loadImg(src, false);
  if (plain) {
    return {
      source: plain,
      width: plain.naturalWidth,
      height: plain.naturalHeight,
      tainted: !isReadable(plain),
    };
  }
  return null;
}
