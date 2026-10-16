/**
 * 光标特效引擎 Composable
 *
 * 用第三方库 ba-click-fx 驱动全局鼠标特效（WebGL2，不可用时库内自动回退
 * Canvas2D），替代内置的 Canvas2D 实现（src/components/effects/CursorEffects.vue）。
 * 库自己会往 document.body 追加一个 pointer-events: none 的全屏覆盖层，所以这里
 * 没有对应的组件与 DOM；旧实现由 App.vue 按设置用 v-if 挂载。
 *
 * 在 App.vue 中调用一次以激活。
 */
import { onBeforeUnmount, ref, watch } from "vue";
import type { BAClickFX } from "ba-click-fx";
import { useSettingsStore } from "@/stores/modules/settings";

/** 新版引擎是否可用；动态导入或初始化失败时置 false，App.vue 据此回退内置实现 */
export const cursorFxBaAvailable = ref(true);

/**
 * 观感微调。库的默认参数取自游戏原版 FX_Touch：拖尾按最小间距抽稀采样点，
 * 泛光按 HDR 场景取值，搬到桌面窗口里偏折线、偏亮。这里按本项目的观感收一档。
 * 参数路径见库导出的 FX_PARAM_SCHEMA（共 66 项，分 hit/flare/disk/rings/shards/trail/bloom 七组）。
 */
const LOOK_PATCH: Record<string, number> = {
  // 拖尾曲线：采样点间距越小越顺，拐角顶点越多越圆
  "trail.minVertexDistance": 12, // 默认 5.4
  "trail.numCornerVertices": 8, // 默认 4
  // 发光：整体泛光强度与拖尾投入泛光的能量
  "bloom.intensity": 1.2, // 默认 1.7
  "bloom.trailEmission": 15, // 默认 23.97
  // 点击时的圆环与碎片亮度
  "rings.hdrIntensity": 6, // 默认 5.99
  "shards.hdrIntensity": 6, // 默认 5.99
};

export function useCursorFx() {
  const settingsStore = useSettingsStore();

  let fx: BAClickFX | null = null;
  let loading = false;

  // 选到内置实现、或两个开关都关掉时不持有实例：既不下载库也不起 WebGL 上下文
  const shouldRun = () =>
    settingsStore.cursorEffectEngine !== "legacy" &&
    (settingsStore.globalMouseTrailEnabled || settingsStore.clickAnimationEnabled);

  const stop = () => {
    fx?.destroy();
    fx = null;
  };

  const start = async () => {
    if (fx || loading || !cursorFxBaAvailable.value) return;
    loading = true;
    try {
      const { BAClickFX } = await import("ba-click-fx");
      // 等待 chunk 期间用户可能已经切走或关掉了开关
      if (!shouldRun() || fx) return;
      fx = new BAClickFX({
        trailAlways: true, // 鼠标移动即出拖尾（库默认只在按住拖动时出现）
        clickEnabled: settingsStore.clickAnimationEnabled,
        trailEnabled: settingsStore.globalMouseTrailEnabled,
        // 特效是盖在页面最上层的独立覆盖层，不参与 DOM 场景合成
        outputCompositing: "browser-overlay",
        // 加色发光观感；背景偏亮时自然收敛，适合可换壁纸
        hostCompositing: "screen",
        // 弹窗/模态框上不画特效
        inputFilter: (e) =>
          !(e.target instanceof Element && e.target.closest(".modal-mask, .blur-overlay")),
      });

      const result = fx.setFxParams(LOOK_PATCH);
      if (import.meta.env.DEV) {
        // 开发期挂到 window 上，方便在控制台用
        // __cursorFx.setFxParam("bloom.intensity", 1.2) 实时试参数
        (window as unknown as { __cursorFx?: unknown }).__cursorFx = fx;
        if (result.rejected.length) console.warn("[光标特效] 参数被拒绝", result.rejected);
      }
    } catch (e) {
      console.warn("[光标特效] ba-click-fx 初始化失败，回退内置实现", e);
      cursorFxBaAvailable.value = false;
    } finally {
      loading = false;
    }
  };

  watch(shouldRun, (on) => (on ? void start() : stop()), { immediate: true });

  // 开关变化时就地更新配置，并清掉已经画出来的粒子
  watch(
    () => [settingsStore.globalMouseTrailEnabled, settingsStore.clickAnimationEnabled] as const,
    ([trail, click]) => {
      if (!fx) return;
      fx.updateConfig({ trailEnabled: trail, clickEnabled: click });
      if (!trail) fx.clearTrail();
      if (!click) fx.clear();
    },
  );

  onBeforeUnmount(stop);
}
