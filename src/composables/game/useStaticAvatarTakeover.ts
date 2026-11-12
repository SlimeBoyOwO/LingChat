/**
 * 静态立绘接管：把聊天界面里那层 DOM 立绘交给 WebGL 渲染器重画，
 * 让光影工作台调出来的效果在游戏里也生效。
 *
 * 为什么需要接管：`SceneLightingRenderer` 的角色 pass 要立绘的像素才能算法线、
 * 边缘光、自阴影。Live2D 拿不到每帧位图，所以那条路径只打了背景；
 * 静态立绘（`avatar_mode: image`）有现成的 `<img>`，可以直接喂纹理。
 *
 * 三条硬约束：
 *  1. 矩形必须用 `imageRectInBox()` 还原 —— `<img>` 是 `object-fit: contain` +
 *     `center bottom`，元素盒子里的留白不是人物。拿元素盒子当矩形，人物会被拉扁。
 *  2. 只隐藏两层 `<img>`，不碰 `ImageAcrossFade` 的 overlay 槽 —— 触摸区域在那一层。
 *  3. 任何一步失败都保持 DOM 原样：宁可没光，也不能把立绘弄没。
 */

import { onBeforeUnmount, ref, shallowRef, type Ref, type ShallowRef } from "vue";

import {
  fitFromObjectFit,
  imageRectInBox,
  type Fit,
} from "@/components/game/standard/touch-regions";

/** `[x, y, width, height]`，单位为舞台像素。 */
export type Rect4 = [number, number, number, number];

/** 出问题时把立绘还给 DOM 的开关：`localStorage.setItem('lighting.takeover', '0')`。 */
const KILL_SWITCH_KEY = "lighting.takeover";

/** 立绘容器标记，由 `StaticRolePresentation` 透出。 */
const CONTAINER_SELECTOR = "[data-lighting-avatar]";

/** 取矩形的节奏。立绘有呼吸动画（1.0035 的缩放），不重测会慢慢偏。 */
const TICK_MS = 33;

/** 矩形变化小于这个值（舞台像素）就不重绘，避免每帧白烧 GPU。 */
const RECT_EPSILON = 0.5;

export type TakeoverStatus = "active" | "inactive" | "fail";

export interface StaticAvatarTakeoverOptions {
  /** 是否允许接管：有光源 + 舞台已启用。 */
  enabled: () => boolean;
  /** 舞台画布。矩形要换算进它的坐标系，dpr 与祖先缩放也从它身上量。 */
  canvas: () => HTMLCanvasElement | null;
  /** 优先接管哪个角色；找不到就取第一个有可见立绘的。 */
  roleId?: () => string | number | null | undefined;
  /** 状态回调，只用于日志。 */
  onStatus?: (status: TakeoverStatus, detail?: string) => void;
}

export interface StaticAvatarTakeover {
  /** 是否已接管（DOM 图已隐藏、canvas 在画人物）。 */
  active: Ref<boolean>;
  /** 交给 `renderer.setCharacter()` 的图。 */
  image: ShallowRef<HTMLImageElement | null>;
  /** 立绘内容矩形（舞台像素），交给 `render()` 的 `characterRect`。 */
  rect: ShallowRef<Rect4 | null>;
  /** 当前接管的图片来源，用来做法线/深度贴图缓存键。 */
  source: Ref<string>;
  /** 按当前状态启动或释放。启用条件变化、场景切换后调一次。 */
  sync: () => Promise<void>;
  /** 强制释放（组件卸载时自己会调）。 */
  release: () => void;
}

function killSwitchOff(): boolean {
  try {
    return localStorage.getItem(KILL_SWITCH_KEY) === "0";
  } catch {
    return false;
  }
}

/** 不透明的那一层。淡入期间两层同时存在，取更清晰的那张。 */
function pickVisibleImage(container: HTMLElement): HTMLImageElement | null {
  const imgs = Array.from(container.querySelectorAll<HTMLImageElement>("img"));
  let best: HTMLImageElement | null = null;
  let bestOpacity = -1;
  for (const img of imgs) {
    if (!img.complete || !img.naturalWidth) continue;
    const opacity = Number.parseFloat(getComputedStyle(img).opacity) || 0;
    if (opacity > bestOpacity) {
      best = img;
      bestOpacity = opacity;
    }
  }
  return best;
}

function loadImage(src: string): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const img = new Image();
    // asset 协议带 ACAO，开 anonymous 才能 `texImage2D` 不被判脏画布
    img.crossOrigin = "anonymous";
    img.onload = () => resolve(img.naturalWidth ? img : null);
    img.onerror = () => resolve(null);
    img.src = src;
  });
}

export function useStaticAvatarTakeover(
  options: StaticAvatarTakeoverOptions,
): StaticAvatarTakeover {
  const active = ref(false);
  const image = shallowRef<HTMLImageElement | null>(null);
  const rect = shallowRef<Rect4 | null>(null);
  const source = ref("");

  let container: HTMLElement | null = null;
  let hiddenImages: HTMLImageElement[] = [];
  let timer = 0;
  let loadToken = 0;
  let loadedSrc = "";
  let lastRectKey = "";

  function status(next: TakeoverStatus, detail?: string) {
    options.onStatus?.(next, detail);
  }

  function findContainer(): HTMLElement | null {
    const all = Array.from(document.querySelectorAll<HTMLElement>(CONTAINER_SELECTOR));
    if (!all.length) return null;

    const want = options.roleId?.() ?? null;
    if (want) {
      const hit = all.find((el) => el.dataset.roleId === String(want));
      if (hit) return hit;
    }
    return all.find((el) => pickVisibleImage(el)) ?? all[0];
  }

  /**
   * 立绘内容矩形，单位为舞台像素（canvas backing store）。
   *
   * 注意量的是 `<img>` 自己的矩形，不是容器：`ImageAcrossFade` 已经把
   * 适配后的尺寸算在 `<img>` 上了（实测容器 589×790、`<img>` 589×785）。
   * 按容器套 contain 会再缩一次，人物整体往下偏 5px。
   * 这里对 img 自身再跑一遍 `imageRectInBox`：比例相符时是恒等变换，
   * 若某套衣服用了 `auto NN%` 之类的取景方式，也能兜住。
   *
   * 坐标必须换算进画布自己的坐标系。全局缩放是 `transform: scale` 打在 `#app` 上
   * （见 `useZoom`），`getBoundingClientRect()` 给的是缩放后的视觉坐标，而画布
   * backing store 按布局像素 × dpr 分配 —— 不除掉缩放，缩放不是 100% 时人物矩形
   * 会整体偏大偏位。投屏窗口的角色层还额外套了一层 `translateX`，所以要先减画布原点。
   */
  function measure(
    img: HTMLImageElement,
    natural?: { width: number; height: number },
  ): Rect4 | null {
    const canvas = options.canvas();
    if (!canvas || !canvas.clientWidth) return null;
    // 几何取 DOM 里那个 img 的盒子；尺寸比例优先用传入的自然尺寸
    // （刚 load 出来的 Image 不在文档里，getBoundingClientRect 全是 0）
    const box = img.getBoundingClientRect();
    if (box.width < 2 || box.height < 2) return null;
    const nw = natural?.width || img.naturalWidth;
    const nh = natural?.height || img.naturalHeight;
    if (!nw || !nh) return null;

    const cRect = canvas.getBoundingClientRect();
    // 画布视觉宽 / 布局宽 = 祖先 transform 的缩放，没有缩放时就是 1
    const zoom = cRect.width / canvas.clientWidth || 1;
    const dpr = canvas.width / canvas.clientWidth || 1;

    // 还原成画布坐标系下的布局像素
    const w = box.width / zoom;
    const h = box.height / zoom;
    const left = (box.left - cRect.left) / zoom;
    const top = (box.top - cRect.top) / zoom;

    const fit: Fit = fitFromObjectFit(getComputedStyle(img).objectFit || "contain");
    // 宽高比不受缩放影响，inner 拿到的是相对比例
    const inner = imageRectInBox(w / h, nw / nh, fit);

    return [
      (left + inner.x * w) * dpr,
      (top + inner.y * h) * dpr,
      inner.width * w * dpr,
      inner.height * h * dpr,
    ];
  }

  function rectKey(r: Rect4): string {
    return r.map((v) => Math.round(v)).join(",");
  }

  function changed(a: Rect4 | null, b: Rect4): boolean {
    if (!a) return true;
    return (
      Math.abs(a[0] - b[0]) > RECT_EPSILON ||
      Math.abs(a[1] - b[1]) > RECT_EPSILON ||
      Math.abs(a[2] - b[2]) > RECT_EPSILON ||
      Math.abs(a[3] - b[3]) > RECT_EPSILON
    );
  }

  function hide(): void {
    if (!container) return;
    hiddenImages = Array.from(container.querySelectorAll<HTMLImageElement>("img"));
    for (const img of hiddenImages) img.style.visibility = "hidden";
  }

  /** 把藏起来的立绘还给 DOM。`hide()` 会整体重写 `hiddenImages`，所以换容器前必须先调它。 */
  function restore(): void {
    for (const img of hiddenImages) img.style.visibility = "";
    hiddenImages = [];
  }

  function release(): void {
    if (timer) {
      window.clearInterval(timer);
      timer = 0;
    }
    loadToken++;
    restore();
    container = null;
    loadedSrc = "";
    lastRectKey = "";
    if (active.value) status("inactive");
    active.value = false;
    image.value = null;
    rect.value = null;
    source.value = "";
  }

  /** 表情/换装换了图：重新灌纹理，别让画布停在旧立绘上。 */
  async function reload(src: string): Promise<void> {
    const token = ++loadToken;
    const loaded = await loadImage(src);
    if (token !== loadToken) return;
    if (!loaded) {
      status("fail", `立绘加载失败 ${src}`);
      release();
      return;
    }
    loadedSrc = src;
    image.value = loaded;
    source.value = src;
    const domImg = container ? pickVisibleImage(container) : null;
    if (domImg) {
      const next = measure(domImg, loaded);
      if (next) {
        rect.value = next;
        lastRectKey = rectKey(next);
      }
    }
  }

  function startTicker(): void {
    if (timer) return;
    timer = window.setInterval(() => {
      if (!active.value || !container) return;

      const img = pickVisibleImage(container);
      if (!img) return;
      if (img.currentSrc && img.currentSrc !== loadedSrc) {
        void reload(img.currentSrc);
        return;
      }

      const next = measure(img, image.value ?? undefined);
      if (!next) return;
      const key = rectKey(next);
      if (key === lastRectKey && !changed(rect.value, next)) return;
      lastRectKey = key;
      rect.value = next;
    }, TICK_MS);
  }

  async function sync(): Promise<void> {
    const want = options.enabled() && !killSwitchOff();
    if (!want) {
      if (active.value) release();
      return;
    }

    const el = findContainer();
    if (!el) {
      if (active.value) release();
      status("inactive", "页面上没有静态立绘容器（Live2D 或未挂载）");
      return;
    }

    // 已经在同一个容器上跑着，只补一次测量
    if (active.value && container === el && image.value) {
      const domImg = pickVisibleImage(el);
      const next = domImg ? measure(domImg, image.value) : null;
      if (next && changed(rect.value, next)) {
        rect.value = next;
        lastRectKey = rectKey(next);
      }
      return;
    }

    const img = pickVisibleImage(el);
    if (!img) {
      status("inactive", "立绘图还没解码完");
      return;
    }

    const src = img.currentSrc || img.src;
    if (!src) {
      status("inactive", "立绘没有 src");
      return;
    }

    const token = ++loadToken;
    const loaded = await loadImage(src);
    if (token !== loadToken) return;
    if (!loaded) {
      status("fail", `立绘加载失败 ${src}`);
      return;
    }

    const next = measure(img, loaded);
    if (!next) {
      status("inactive", "立绘盒子尺寸为 0（还没布局）");
      return;
    }

    // 先量再藏：上一步任何失败，DOM 立绘都还是原样
    // 切到另一个容器（换角色）时，上一个容器的 <img> 仍挂在 DOM 上，
    // 不还回去它就永远停在 visibility:hidden —— 表现为切角色后前一个立绘凭空消失
    if (container && container !== el) restore();
    container = el;
    loadedSrc = src;
    image.value = loaded;
    source.value = src;
    rect.value = next;
    lastRectKey = rectKey(next);
    hide();
    active.value = true;
    status(
      "active",
      `${Math.round(next[2])}x${Math.round(next[3])} @ ${Math.round(next[0])},${Math.round(next[1])}`,
    );
    startTicker();
  }

  onBeforeUnmount(release);

  return { active, image, rect, source, sync, release };
}
