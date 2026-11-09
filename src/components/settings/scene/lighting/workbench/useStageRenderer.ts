/**
 * 画布渲染接线。
 *
 * 图片加载是这里最容易出事的一环，规则写死：
 * 1. **先按 CORS 方式加载**（`crossOrigin="anonymous"`）。Tauri 的 asset:// 支持带 ACAO，
 *    这样拿到的图是「干净」的，WebGL 能上传、也能读像素生成法线/深度。
 * 2. 失败再退回普通加载，并**探测画布是否被污染**：污染图 `texImage2D` 会直接抛
 *    SecurityError，等于整块画布全黑——所以宁可不画，也不能把脏图喂给 WebGL。
 * 3. 两条都不行再试 `fetch → createImageBitmap`（换一条路拿干净位图）。
 * 任何一步失败都不抛到组件外，只记 `lastError`，由界面显示原因。
 */
import { onMounted, onUnmounted, ref, watch } from "vue";
import type {
  LightSource,
  LightingCameraParams,
  LightingCompositingParams,
  LightingGlobalParams,
  LightingMaterialParams,
} from "@/api/services/scene";
import { SceneLightingRenderer } from "@/render/lighting";
import { calcBgRect, calcCharRect, type Size } from "./layoutStage";

/** 舞台固定分辨率：渲染内核按这个尺寸建 FBO，显示尺寸交给 CSS。 */
export const STAGE_W = 960;
export const STAGE_H = 540;

/** 占位底图直接用 2D canvas 画：SVG 图片在部分 WebView 里 texImage2D 会失败 */
function makeFallbackCanvas(w = STAGE_W, h = STAGE_H): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  const ctx = c.getContext("2d");
  if (ctx) {
    ctx.fillStyle = "#e9e9ef";
    ctx.fillRect(0, 0, w, h);
    ctx.fillStyle = "#a8a8b4";
    ctx.font = "20px sans-serif";
    ctx.textAlign = "center";
    ctx.fillText("暂无可用场景底图", w / 2, h / 2);
  }
  return c;
}

export interface StageRendererInput {
  backgroundSrc: () => string | null | undefined;
  characterSrc: () => string | null | undefined;
  lights: () => LightSource[];
  globalParams: () => LightingGlobalParams;
  compositingParams: () => LightingCompositingParams;
  materialParams: () => LightingMaterialParams;
  cameraParams: () => LightingCameraParams;
  compareOriginal: () => boolean;
  normalSrc?: () => string | null | undefined;
  depthSrc?: () => string | null | undefined;
}

interface LoadedImage {
  source: TexImageSource;
  width: number;
  height: number;
  /** true = 画布被污染，WebGL 不能上传 */
  tainted: boolean;
}

function loadImg(src: string, anonymous: boolean): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const img = new Image();
    if (anonymous) img.crossOrigin = "anonymous";
    img.onload = () => resolve(img);
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

async function loadSmart(src: string): Promise<LoadedImage | null> {
  // 1) 带 CORS 的图片（Tauri asset:// 若允许 ACAO，这条就成）
  const anon = await loadImg(src, true);
  if (anon) {
    return { source: anon, width: anon.naturalWidth, height: anon.naturalHeight, tainted: false };
  }
  // 2) fetch 取字节再解码：位图天然是「干净」的，WebGL 和读像素都不受跨源限制
  try {
    const res = await fetch(src);
    if (res.ok) {
      const bmp = await createImageBitmap(await res.blob());
      return { source: bmp, width: bmp.width, height: bmp.height, tainted: false };
    }
  } catch {
    /* 跨源且无 ACAO 时 fetch 也会失败，继续往下 */
  }
  // 3) 普通加载：能显示，但可能污染画布 —— 交给调用方判断，绝不喂给 WebGL
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

export function useStageRenderer(input: StageRendererInput) {
  const canvasRef = ref<HTMLCanvasElement | null>(null);
  const isComparing = ref(false);
  const bgSize = ref<Size>({ width: STAGE_W, height: STAGE_H });
  /** 最近一次失败原因（中文可读），空串表示正常 */
  const lastError = ref("");
  /** 资源级错误（底图/立绘不可用），只有成功换图才清掉 */
  const resourceError = ref("");
  const displayError = () => {
    lastError.value = resourceError.value || (renderer ? renderer.lastError : "");
  };

  let renderer: SceneLightingRenderer | null = null;
  let bgImg: TexImageSource | null = null;
  let charImg: TexImageSource | null = null;
  let disposed = false;
  let bgToken = 0;
  let charToken = 0;
  let rafId = 0;

  const customNormalImg = { src: "", img: null as TexImageSource | null };
  const customDepthImg = { src: "", img: null as TexImageSource | null };

  function ensureMapsIfNeeded() {
    const nSrc = input.normalSrc?.() || "";
    const dSrc = input.depthSrc?.() || "";
    const jobs: Promise<void>[] = [];
    if (nSrc !== customNormalImg.src) {
      customNormalImg.src = nSrc;
      customNormalImg.img = null;
      if (nSrc) {
        jobs.push(
          loadSmart(nSrc).then((r) => {
            customNormalImg.img = r && !r.tainted ? r.source : null;
          }),
        );
      }
    }
    if (dSrc !== customDepthImg.src) {
      customDepthImg.src = dSrc;
      customDepthImg.img = null;
      if (dSrc) {
        jobs.push(
          loadSmart(dSrc).then((r) => {
            customDepthImg.img = r && !r.tainted ? r.source : null;
          }),
        );
      }
    }
    return Promise.all(jobs);
  }

  /** 把立绘重新喂给渲染器（含上传的法线/深度）。失败只记日志，不抛。 */
  async function applyCharacter() {
    if (!renderer || !charImg) return;
    await ensureMapsIfNeeded();
    if (!renderer || !charImg) return;
    try {
      renderer.setCharacter(charImg, {
        normal: customNormalImg.img ?? "auto",
        depth: customDepthImg.img ?? "auto",
      });
      lastError.value = renderer.lastError;
      scheduleRender();
    } catch (err) {
      lastError.value = `立绘贴图应用失败：${(err as Error).message.slice(0, 60)}`;
      console.warn("[lighting] 立绘贴图应用失败:", err);
    }
  }

  async function mount() {
    if (!canvasRef.value) return;
    try {
      renderer = new SceneLightingRenderer(canvasRef.value);
      renderer.resize(STAGE_W, STAGE_H);
    } catch (err) {
      lastError.value = `WebGL 初始化失败：${(err as Error).message.slice(0, 60)}`;
      return;
    }

    // ---- 背景 ----
    const bgSrc = input.backgroundSrc();
    const bgLoaded = bgSrc ? await loadSmart(bgSrc) : null;
    if (disposed || !renderer) return;

    if (bgLoaded && !bgLoaded.tainted) {
      try {
        renderer.setBackground(bgLoaded.source);
        bgImg = bgLoaded.source;
        if (bgLoaded.width && bgLoaded.height) {
          bgSize.value = { width: bgLoaded.width, height: bgLoaded.height };
        }
      } catch (err) {
        resourceError.value = `场景底图不可用：${(err as Error).message.slice(0, 60)}`;
      }
    } else if (bgLoaded?.tainted) {
      resourceError.value = "场景底图跨源受限，无法上传到 WebGL；请用「换图」选本地图片";
      displayError();
    }

    // 背景彻底拿不到就用占位底图，保证画布不是全黑（且不会因为 SVG 上传失败）
    if (!bgImg && renderer) {
      const fallback = makeFallbackCanvas();
      try {
        renderer.setBackground(fallback);
        bgImg = fallback;
      } catch (err) {
        resourceError.value = `占位底图也不可用：${(err as Error).message.slice(0, 50)}`;
      }
    }
    displayError();

    // ---- 立绘 ----
    const charSrc = input.characterSrc();
    if (charSrc) {
      const ch = await loadSmart(charSrc);
      if (disposed || !renderer) return;
      if (ch && !ch.tainted) {
        charImg = ch.source;
        await applyCharacter();
      } else if (ch?.tainted) {
        resourceError.value = "立绘跨源受限，无法上传到 WebGL";
        displayError();
      }
    }

    renderFrame();
  }

  function scheduleRender() {
    if (rafId) return;
    rafId = requestAnimationFrame(() => {
      rafId = 0;
      renderFrame();
    });
  }

  function renderFrame() {
    if (!renderer) return;
    const camera = input.cameraParams();
    const material = input.materialParams();
    const camZoom = camera.zoom || 1.25;
    const camFocusY = camera.focus_y ?? 0.16;

    const bgRect = calcBgRect(bgSize.value, STAGE_W, STAGE_H, camZoom, camFocusY);
    const charAspect =
      charImg && (charImg as { width?: number }).width && (charImg as { height?: number }).height
        ? (charImg as { width: number }).width / (charImg as { height: number }).height
        : 0.75;
    const charRect = calcCharRect(charAspect, STAGE_W, STAGE_H, camZoom, camFocusY);

    try {
      if (input.compareOriginal() || isComparing.value) {
        renderer.render({
          stage: { width: STAGE_W, height: STAGE_H },
          backgroundRect: bgRect,
          characterRect: charRect,
          lights: [],
          global: {
            master: 1,
            ambient_blend: 0,
            albedo_protect: 0,
            soft_knee: 0.7,
            lift: 0,
            mono: 0,
          },
          compositing: { light_wrap: 0, rim_width: 0, bloom_int: 0, bloom_thresh: 1 },
          material: { ...material, use_normal: false, use_depth: false },
          camera,
        });
        return;
      }

      renderer.render({
        stage: { width: STAGE_W, height: STAGE_H },
        backgroundRect: bgRect,
        characterRect: charRect,
        lights: input.lights().map((l) => ({ ...l, enabled: l.enabled !== false })),
        global: input.globalParams(),
        compositing: input.compositingParams(),
        material,
        camera,
      });
      displayError();
    } catch (err) {
      // 一帧画挂不该把整块画布打死：记下原因，保留上一帧
      lastError.value = `渲染失败：${(err as Error).message.slice(0, 60)}`;
      console.warn("[lighting] 渲染失败:", err);
    }
  }

  watch(
    () => [
      input.lights(),
      input.globalParams(),
      input.compositingParams(),
      input.materialParams(),
      input.cameraParams(),
      input.compareOriginal(),
      isComparing.value,
    ],
    () => scheduleRender(),
    { deep: true },
  );

  watch(
    () => input.backgroundSrc(),
    async (src) => {
      if (!renderer || !src) return;
      const token = ++bgToken;
      const loaded = await loadSmart(src);
      if (token !== bgToken || disposed || !renderer) return;
      if (!loaded) return;
      if (loaded.tainted) {
        resourceError.value = "场景底图跨源受限，无法上传到 WebGL；请用「换图」选本地图片";
        displayError();
        return;
      }
      try {
        renderer.setBackground(loaded.source);
        bgImg = loaded.source;
        resourceError.value = "";
        if (loaded.width && loaded.height) {
          bgSize.value = { width: loaded.width, height: loaded.height };
        }
        lastError.value = renderer.lastError;
        scheduleRender();
      } catch (err) {
        resourceError.value = `场景底图不可用：${(err as Error).message.slice(0, 60)}`;
        displayError();
      }
    },
  );

  watch(
    () => input.characterSrc(),
    async (src) => {
      if (!renderer) return;
      const token = ++charToken;
      if (!src) {
        if (token === charToken) renderer.clearCharacter();
        scheduleRender();
        return;
      }
      const ch = await loadSmart(src);
      if (token !== charToken || disposed || !renderer) return;
      if (ch && !ch.tainted) {
        charImg = ch.source;
        await applyCharacter();
      } else if (ch?.tainted) {
        resourceError.value = "立绘跨源受限，无法上传到 WebGL";
        displayError();
      }
    },
  );

  watch([() => input.normalSrc?.(), () => input.depthSrc?.()], () => {
    void applyCharacter();
  });

  onMounted(mount);

  onUnmounted(() => {
    disposed = true;
    if (rafId) {
      cancelAnimationFrame(rafId);
      rafId = 0;
    }
    bgImg = null;
    charImg = null;
    renderer?.dispose();
    renderer = null;
  });

  return { canvasRef, isComparing, bgSize, lastError, resourceError, scheduleRender };
}
