/**
 * 画布渲染接线。
 *
 * 图片加载统一走 `loadTexImage`（见 @/render/lighting/loadImage）：先按 CORS 加载，
 * 失败退 fetch 解码，再失败才普通加载并探测画布是否被污染 —— 污染图喂给 WebGL
 * 会让整块画布全黑，宁可不画。任何一步失败都不抛到组件外，只记 `lastError`，
 * 由界面显示原因。
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
import { loadTexImage } from "@/render/lighting/loadImage";
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
  let normalToken = 0;
  let depthToken = 0;
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
        const token = ++normalToken;
        jobs.push(
          loadTexImage(nSrc).then((r) => {
            // 快速切换时旧请求可能后到，会把旧图写给新 src
            if (token !== normalToken) return;
            customNormalImg.img = r && !r.tainted ? r.source : null;
          }),
        );
      }
    }
    if (dSrc !== customDepthImg.src) {
      customDepthImg.src = dSrc;
      customDepthImg.img = null;
      if (dSrc) {
        const token = ++depthToken;
        jobs.push(
          loadTexImage(dSrc).then((r) => {
            if (token !== depthToken) return;
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
    // 占住 token：等待期间 backgroundSrc 的 watcher 可能也在加载同一张或另一张图，
    // 不比对就会让 mount 的旧结果盖掉 watcher 的新结果
    const bgSrc = input.backgroundSrc();
    const bgTokenAtMount = ++bgToken;
    const bgLoaded = bgSrc ? await loadTexImage(bgSrc) : null;
    if (disposed || !renderer || bgTokenAtMount !== bgToken) return;

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
      resourceError.value = "场景底图跨源受限，无法上传到 WebGL";
      displayError();
    } else {
      resourceError.value = "场景底图加载失败，已回退到占位底图";
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
      const charTokenAtMount = ++charToken;
      const ch = await loadTexImage(charSrc);
      if (disposed || !renderer || charTokenAtMount !== charToken) return;
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
          characterRects: [charRect],
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
        characterRects: [charRect],
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
      const loaded = await loadTexImage(src);
      if (token !== bgToken || disposed || !renderer) return;
      if (!loaded) return;
      if (loaded.tainted) {
        resourceError.value = "场景底图跨源受限，无法上传到 WebGL";
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
      const ch = await loadTexImage(src);
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
