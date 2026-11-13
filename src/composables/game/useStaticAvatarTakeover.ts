/**
 * 静态立绘接管：把聊天界面里那层 DOM 立绘交给 WebGL 渲染器重画，
 * 让光影工作台调出来的效果在游戏里也生效。
 *
 * 为什么需要接管：`SceneLightingRenderer` 的角色 pass 要立绘的像素才能算法线、
 * 边缘光、自阴影。Live2D 拿不到每帧位图，所以那条路径只打了背景；
 * 静态立绘（`avatar_mode: image`）有现成的 `<img>`，可以直接喂纹理。
 *
 * 多角色：每个在场角色各占一个槽，各自找容器、量矩形、灌纹理。槽序 = 传入的
 * `roleIds()` 顺序（即舞台前后序，0 在最底层），与渲染器 `characterRects` 下标对齐。
 * 某个角色接管失败只影响它自己，其余角色照常吃光。
 *
 * 三条硬约束：
 *  1. 矩形必须用 `imageRectInBox()` 还原 —— `<img>` 是 `object-fit: contain` +
 *     `center bottom`，元素盒子里的留白不是人物。拿元素盒子当矩形，人物会被拉扁。
 *  2. 只隐藏 `<img>`，不碰 `ImageAcrossFade` 的 overlay 槽 —— 触摸区域在那一层。
 *     隐藏必须反复补：`ImageAcrossFade` 换图是「销毁旧 img、按 v-if 新建 img」，
 *     接管那一刻的快照下一帧就全废了。
 *  3. 任何一步失败都保持 DOM 原样：宁可没光，也不能把立绘弄没。
 *  4. 绑定必须跟着角色上下场走：容器被 Vue 卸载（切角色 / 角色退场）时立刻释放。
 *     画布上那份纹理没人管下线的话，上一任角色就会「钉」在场景上不走。
 */

import { onBeforeUnmount, ref, shallowRef, type Ref, type ShallowRef } from "vue";

import {
  fitFromObjectFit,
  imageRectInBox,
  type Fit,
} from "@/components/game/standard/touch-regions";
import { loadTexImage, type LoadedImage } from "@/render/lighting/loadImage";
import { MAX_CHARACTERS } from "@/render/lighting";

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
  /** 在场角色 id，按舞台前后序（0 在最底层）。超出渲染器槽上限的截断。 */
  roleIds: () => readonly (string | number)[];
  /** 状态回调，只用于日志。 */
  onStatus?: (status: TakeoverStatus, detail?: string) => void;
}

/** 单个角色槽的对外快照。下标即渲染器槽号。 */
export interface TakeoverSlot {
  roleId: string;
  active: boolean;
  image: LoadedImage | null;
  rect: Rect4 | null;
  source: string;
}

export interface StaticAvatarTakeover {
  /** 是否有任一槽已接管（DOM 图已隐藏、canvas 在画人物）。 */
  active: Ref<boolean>;
  /** 各角色槽快照，按 `roleIds()` 序；槽号 = 下标。 */
  slots: ShallowRef<readonly TakeoverSlot[]>;
  /** 按当前状态启动或释放。启用条件变化、角色上下场后调一次。 */
  sync: () => Promise<void>;
  /** 强制释放全部槽（组件卸载时自己会调）。 */
  release: () => void;
}

/** 槽的内部可变状态。对外只发快照，避免渲染器读到半截状态。 */
interface SlotState {
  roleId: string;
  container: HTMLElement | null;
  hiddenImages: HTMLImageElement[];
  loadToken: number;
  loadedSrc: string;
  lastRectKey: string;
  active: boolean;
  image: LoadedImage | null;
  rect: Rect4 | null;
  source: string;
}

function killSwitchOff(): boolean {
  try {
    return localStorage.getItem(KILL_SWITCH_KEY) === "0";
  } catch {
    return false;
  }
}

/**
 * 图层有效不透明度：从 img 一路乘到容器。
 *
 * `ImageAcrossFade` 把渐变打在**层 div** 上，img 自己的 opacity 恒为 1；而瞬切
 * 路径（`needsInstantSwap` 判定为换姿势）不清空旧层，两层会同时挂着 img。
 * 只看 img 自己的 opacity 就会挑中那张已经淡出的旧图 —— 画布停在上一张表情上，
 * 之后换回原来的表情也追不回来。
 */
function effectiveOpacity(img: HTMLElement, root: HTMLElement): number {
  let opacity = 1;
  let el: HTMLElement | null = img;
  while (el && el !== root) {
    opacity *= Number.parseFloat(getComputedStyle(el).opacity) || 0;
    el = el.parentElement;
  }
  return opacity;
}

/** 不透明的那一层。淡入期间两层同时存在，取更清晰的那张。 */
function pickVisibleImage(container: HTMLElement): HTMLImageElement | null {
  const imgs = Array.from(container.querySelectorAll<HTMLImageElement>("img"));
  let best: HTMLImageElement | null = null;
  let bestOpacity = -1;
  for (const img of imgs) {
    if (!img.complete || !img.naturalWidth) continue;
    const opacity = effectiveOpacity(img, container);
    if (opacity > bestOpacity) {
      best = img;
      bestOpacity = opacity;
    }
  }
  return best;
}

export function useStaticAvatarTakeover(
  options: StaticAvatarTakeoverOptions,
): StaticAvatarTakeover {
  const active = ref(false);
  const slots = shallowRef<readonly TakeoverSlot[]>([]);

  let states: SlotState[] = [];
  let timer = 0;
  let lastSig = "";

  function status(next: TakeoverStatus, detail?: string) {
    options.onStatus?.(next, detail);
  }

  function publish(): void {
    // 心跳每轮都调 sync，但状态没变就别发新快照，免得舞台白重绘
    const sig = states
      .map((s) => `${s.roleId}:${s.active ? 1 : 0}:${s.source}:${s.rect ? rectKey(s.rect) : ""}`)
      .join("|");
    if (sig === lastSig) return;
    lastSig = sig;
    slots.value = states.map((s) => ({
      roleId: s.roleId,
      active: s.active,
      image: s.image,
      rect: s.rect,
      source: s.source,
    }));
    active.value = states.some((s) => s.active);
  }

  function findContainer(roleId: string): HTMLElement | null {
    const all = document.querySelectorAll<HTMLElement>(CONTAINER_SELECTOR);
    for (const el of all) {
      if (el.dataset.roleId === roleId) return el;
    }
    return null;
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

  /**
   * 把容器里的 `<img>` 全藏起来，已藏过的跳过。
   *
   * 角色层绘制在光影画布之上，漏一张就是「场景吃光、人物不吃光」：画布里那份
   * 打光的立绘被 DOM 里这份原图盖住。换表情必然新建 img，所以每次 tick 都要补。
   */
  function ensureHidden(s: SlotState): void {
    if (!s.container) return;
    for (const img of s.container.querySelectorAll<HTMLImageElement>("img")) {
      if (img.style.visibility === "hidden") continue;
      img.style.visibility = "hidden";
      s.hiddenImages.push(img);
    }
  }

  /** 把藏起来的立绘还给 DOM。释放槽前必须先调它，否则那个角色会永久隐身。 */
  function restore(s: SlotState): void {
    for (const img of s.hiddenImages) img.style.visibility = "";
    s.hiddenImages = [];
  }

  function releaseSlot(s: SlotState): void {
    s.loadToken++;
    restore(s);
    s.container = null;
    s.loadedSrc = "";
    s.lastRectKey = "";
    s.active = false;
    s.image = null;
    s.rect = null;
    s.source = "";
  }

  function release(): void {
    if (timer) {
      window.clearInterval(timer);
      timer = 0;
    }
    const wasActive = active.value;
    for (const s of states) releaseSlot(s);
    states = [];
    publish();
    if (wasActive) status("inactive");
  }

  /** 表情/换装换了图：重新灌纹理，别让画布停在旧立绘上。只影响这一个槽。 */
  async function reload(s: SlotState, src: string): Promise<void> {
    const token = ++s.loadToken;
    const loaded = await loadTexImage(src);
    if (token !== s.loadToken || !states.includes(s)) return;
    if (!loaded || loaded.tainted) {
      status("fail", loaded ? `立绘图会污染画布 ${src}` : `立绘加载失败 ${src}`);
      releaseSlot(s);
      publish();
      return;
    }
    s.loadedSrc = src;
    s.image = loaded;
    s.source = src;
    const domImg = s.container ? pickVisibleImage(s.container) : null;
    if (domImg) {
      const next = measure(domImg, loaded);
      if (next) {
        s.rect = next;
        s.lastRectKey = rectKey(next);
      }
    }
    publish();
  }

  /** 已绑定的槽补一次隐藏和测量；矩形真动了才发快照。 */
  function remeasure(s: SlotState): void {
    if (!s.container) return;
    ensureHidden(s);
    const domImg = pickVisibleImage(s.container);
    const next = domImg ? measure(domImg, s.image ?? undefined) : null;
    if (next && changed(s.rect, next)) {
      s.rect = next;
      s.lastRectKey = rectKey(next);
      publish();
    }
  }

  /** 给一个还没绑定的槽找容器、灌纹理、藏图、激活。任何一步失败都保持 DOM 原样。 */
  async function bindSlot(s: SlotState): Promise<void> {
    const el = findContainer(s.roleId);
    if (!el) return;
    const img = pickVisibleImage(el);
    if (!img) return; // 图还在解码，心跳下一轮再补
    const src = img.currentSrc || img.src;
    if (!src) return;

    const token = ++s.loadToken;
    const loaded = await loadTexImage(src);
    if (token !== s.loadToken || !states.includes(s)) return;
    if (!loaded || loaded.tainted) {
      status("fail", loaded ? `立绘图会污染画布 ${src}` : `立绘加载失败 ${src}`);
      return;
    }
    const next = measure(img, loaded);
    if (!next) return; // 盒子尺寸为 0（还没布局），心跳再补

    // 先量再藏：上一步任何失败，DOM 立绘都还是原样
    s.container = el;
    s.loadedSrc = src;
    s.image = loaded;
    s.source = src;
    s.rect = next;
    s.lastRectKey = rectKey(next);
    ensureHidden(s);
    s.active = true;
    status(
      "active",
      `角色 ${s.roleId} ${Math.round(next[2])}x${Math.round(next[3])} @ ${Math.round(next[0])},${Math.round(next[1])}`,
    );
    publish();
    startTicker();
  }

  function startTicker(): void {
    if (timer) return;
    timer = window.setInterval(() => {
      let dirty = false;
      for (const s of states) {
        if (!s.active || !s.container) continue;
        // 容器被卸了（切角色 / 角色退场）：旧纹理立刻下线，别钉在场景上
        if (!s.container.isConnected) {
          releaseSlot(s);
          dirty = true;
          continue;
        }

        // 先补藏再量：新建的 img 晚一帧藏就会盖在画布上闪一下原图
        ensureHidden(s);

        const img = pickVisibleImage(s.container);
        if (!img) continue;
        if (img.currentSrc && img.currentSrc !== s.loadedSrc) {
          void reload(s, img.currentSrc);
          continue;
        }

        const next = measure(img, s.image ?? undefined);
        if (!next) {
          // 量不到矩形 = 图被 display:none（Live2D 模型就绪后藏掉占位立绘）或画布没尺寸。
          // 继续画就是给模型盖一张打光的静态鬼影，立刻下线等心跳重判
          releaseSlot(s);
          dirty = true;
          continue;
        }
        const key = rectKey(next);
        if (key === s.lastRectKey && !changed(s.rect, next)) continue;
        s.lastRectKey = key;
        s.rect = next;
        dirty = true;
      }
      if (dirty) publish();
    }, TICK_MS);
  }

  async function sync(): Promise<void> {
    const want = options.enabled() && !killSwitchOff();
    if (!want) {
      if (states.length) release();
      return;
    }

    // 按当前在场序重建槽列表：留下的角色复用旧状态（纹理不重灌），
    // 退场的角色立刻释放，新上场的补一个空槽
    const ids = options.roleIds().map(String).slice(0, MAX_CHARACTERS);
    const next: SlotState[] = [];
    for (const id of ids) {
      const existing = states.find((s) => s.roleId === id);
      if (existing) {
        next.push(existing);
      } else {
        next.push({
          roleId: id,
          container: null,
          hiddenImages: [],
          loadToken: 0,
          loadedSrc: "",
          lastRectKey: "",
          active: false,
          image: null,
          rect: null,
          source: "",
        });
      }
    }
    for (const s of states) {
      if (!next.includes(s)) releaseSlot(s);
    }
    states = next;

    for (const s of states) {
      // 绑定的容器被 Vue 卸了：先下线再重找，别拿已卸载容器的旧纹理旧矩形
      if (s.active && s.container && !s.container.isConnected) releaseSlot(s);
      if (s.active && s.container && s.image) {
        remeasure(s);
      } else {
        void bindSlot(s);
      }
    }
    publish();
  }

  onBeforeUnmount(release);

  return { active, slots, sync, release };
}
