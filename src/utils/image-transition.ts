const SAMPLE_SIZE = 64;
const CACHE_LIMIT = 32;

export interface ImageProfile {
  aspectRatio: number;
  alpha: Uint8Array;
  opaque: boolean;
}

const profiles = new Map<string, ImageProfile>();

/** 只缓存缩小的透明度轮廓，避免长期保留完整立绘的解码数据。 */
export function imageProfile(url: string, image: HTMLImageElement): ImageProfile | null {
  const cached = profiles.get(url);
  if (cached) {
    profiles.delete(url);
    profiles.set(url, cached);
    return cached;
  }
  try {
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = SAMPLE_SIZE;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) return null;
    context.drawImage(image, 0, 0, SAMPLE_SIZE, SAMPLE_SIZE);
    const pixels = context.getImageData(0, 0, SAMPLE_SIZE, SAMPLE_SIZE).data;
    const alpha = new Uint8Array(SAMPLE_SIZE * SAMPLE_SIZE);
    let transparent = 0;
    for (let i = 0; i < alpha.length; i++) {
      alpha[i] = pixels[i * 4 + 3];
      if (alpha[i] < 240) transparent++;
    }
    const profile = {
      aspectRatio: image.naturalWidth / image.naturalHeight,
      alpha,
      opaque: transparent / alpha.length < 0.01,
    };
    profiles.set(url, profile);
    if (profiles.size > CACHE_LIMIT) profiles.delete(profiles.keys().next().value!);
    return profile;
  } catch {
    // 跨域图片无法读取像素时仍可显示，换图时保守使用瞬切。
    return null;
  }
}

/** 不透明背景保留渐变；立绘取景或透明轮廓差异较大时瞬切，避免双姿势重叠。 */
export function needsInstantSwap(before: ImageProfile | null, after: ImageProfile | null): boolean {
  if (!before || !after) return true;
  if (before.opaque && after.opaque) return false;
  if (Math.abs(before.aspectRatio / after.aspectRatio - 1) > 0.02) return true;
  let intersection = 0;
  let union = 0;
  for (let i = 0; i < before.alpha.length; i++) {
    intersection += Math.min(before.alpha[i], after.alpha[i]);
    union += Math.max(before.alpha[i], after.alpha[i]);
  }
  // 轮廓重合度低于 94% 视为不同姿势；全透明图片不参与除法。
  return union > 0 && intersection / union < 0.94;
}
