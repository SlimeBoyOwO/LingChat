/**
 * 全局路由切换黑屏过渡
 *
 * 单例：模块级状态，在 main.ts 调用一次 installRouteFade(router) 注册守卫，
 * App.vue 常驻挂载 RouteFadeMask.vue 作为遮罩 DOM。
 *
 * 守卫链顺序（组件 beforeRouteLeave -> 全局 beforeEach -> beforeRouteUpdate
 * -> 路由 beforeEnter -> beforeRouteEnter 并在此 await 懒加载 import
 * -> 全局 beforeResolve -> 落地 -> 全局 afterEach -> DOM 更新）。
 * 淡入放在全局 beforeEach 里 await，所以懒加载 chunk 的下载解析耗时天然被全黑盖住；
 * 组件内 beforeRouteLeave（如剧本编辑器离开前的落盘同步）先于本守卫执行，
 * 顺序正好是「先清理完画面还在原页 -> 再黑屏」，无需额外处理。
 */
import { computed, nextTick, ref } from "vue";
import type { RouteLocationNormalized, Router } from "vue-router";

/** 淡入时长，会被 beforeEach await，直接叠加到每次跳转耗时上，不宜调大 */
const FADE_IN_MS = 260;
/** 淡出时长，不阻塞任何逻辑，可以比淡入长 */
const FADE_OUT_MS = 420;
/** 看门狗：遮罩亮起后最多停留这么久，兜底防止异常情况下永久黑屏 */
const WATCHDOG_MS = 4000;
/** 跑在透明或点击穿透窗口里的路由，盖黑会变成桌面上的黑方块，与 useZoom 的 ownScalingRoutes 一致 */
const SKIP_PATHS = new Set(["/pet", "/bubble"]);

/** 遮罩是否可见，RouteFadeMask 直接读 */
export const maskVisible = ref(false);

const reduceMotion =
  typeof window !== "undefined" &&
  typeof window.matchMedia === "function" &&
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

const inMs = () => (reduceMotion ? 0 : FADE_IN_MS);
const outMs = () => (reduceMotion ? 0 : FADE_OUT_MS);

/** 遮罩内联样式：淡入淡出用不同时长与缓动 */
export const maskStyle = computed(() => ({
  transitionDuration: `${maskVisible.value ? inMs() : outMs()}ms`,
  transitionTimingFunction: maskVisible.value ? "ease-in" : "ease-out",
}));

let watchdogTimer: number | null = null;

/** 等一帧，保证遮罩的透明度变化被浏览器真正画出来。
    窗口被最小化或完全遮挡时 rAF 不触发，加超时兜底，避免导航被永久挂起 */
function nextFrame(): Promise<void> {
  return new Promise((resolve) => {
    const timer = window.setTimeout(resolve, 100);
    requestAnimationFrame(() => {
      clearTimeout(timer);
      resolve();
    });
  });
}

/** 亮起遮罩，resolve 时已全黑；可重入，重复调用只是再等一次淡入时长 */
async function showMask(): Promise<void> {
  maskVisible.value = true;
  if (watchdogTimer === null) {
    watchdogTimer = window.setTimeout(() => {
      console.warn("[useRouteFade] 看门狗触发：跳转长时间未收尾，强制收起遮罩");
      hideMask();
    }, inMs() + WATCHDOG_MS);
  }
  if (inMs() > 0) await new Promise((resolve) => setTimeout(resolve, inMs()));
  // 等一帧让不透明度真正上屏，否则路由可能在遮罩被绘制之前就换了
  await nextFrame();
}

/** 收起遮罩，幂等 */
function hideMask(): void {
  if (watchdogTimer !== null) {
    clearTimeout(watchdogTimer);
    watchdogTimer = null;
  }
  maskVisible.value = false;
}

/** 启动期导航与透明窗口路由不做过渡 */
function shouldSkip(to: RouteLocationNormalized, from: RouteLocationNormalized): boolean {
  // from 是 START_LOCATION：main.ts 里独立窗口的 router.replace 与首次导航都走这条分支
  if (from.matched.length === 0) return true;
  return SKIP_PATHS.has(to.path) || SKIP_PATHS.has(from.path);
}

export function installRouteFade(router: Router): void {
  router.beforeEach(async (to, from) => {
    if (shouldSkip(to, from)) return;
    try {
      await showMask();
    } catch (e) {
      // 守卫自身绝不抛错，否则整条守卫链会走 triggerError 分支
      console.warn("[useRouteFade] 淡入失败，跳过过渡:", e);
    }
  });

  router.afterEach((_to, _from, failure) => {
    // 被取消或被顶替的导航不收遮罩：真正落地的那次会收，看门狗兜底
    if (failure) return;
    // 以遮罩当前状态为准，而不是重算跳过条件：被顶替的那次可能已经亮起遮罩，
    // 而落地这次恰好是跳过路由，漏收会一直黑到看门狗超时
    if (!maskVisible.value) return;
    // 等新路由组件挂载并完成首帧布局再淡出，避免露出未渲染完的页面
    void nextTick()
      .then(nextFrame)
      .then(() => hideMask());
  });

  // 守卫抛错或懒加载 chunk 失败时 afterEach 不触发，这里兜底
  router.onError((error, to, from) => {
    // 注册了 onError 之后 vue-router 不再自行 console.error，这里把日志补回
    console.error("[useRouteFade] 路由跳转出错:", error, to, from);
    hideMask();
  });
}
