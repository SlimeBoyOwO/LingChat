import { onBeforeUnmount, onMounted, watch, type Ref } from "vue";
import { FallingField } from "./useFallingField";
import type { FallingEffect, FallingProps } from "../types/falling";

/**
 * 把下落粒子的渲染场挂到画布上：负责尺寸观察、生命周期与可见性。
 *
 * 画布尺寸取自父元素，所以主界面铺满整屏、塞进小容器里也成立。
 * 雪与樱花共用这一份实现，差异只在传入的 effect。
 */
export function useFallingParticle(
  canvasRef: Ref<HTMLCanvasElement | null>,
  props: FallingProps,
  effect: FallingEffect,
) {
  let field: FallingField | null = null;
  let observer: ResizeObserver | null = null;

  const resize = () => field?.resize();

  const onVisibilityChange = () => {
    if (document.hidden) field?.stop();
    else if (props.enabled ?? true) field?.start();
  };

  onMounted(() => {
    const canvas = canvasRef.value;
    if (!canvas) return;

    field = new FallingField(canvas, effect, props.intensity ?? 1);

    // 观察父元素而不是窗口：布局变化（对话框展开、窗口分屏）同样会改变可用区域
    const host = canvas.parentElement;
    if (host && typeof ResizeObserver !== "undefined") {
      observer = new ResizeObserver(resize);
      observer.observe(host);
    } else {
      window.addEventListener("resize", resize);
    }
    document.addEventListener("visibilitychange", onVisibilityChange);

    field.resize();
    if (props.enabled ?? true) field.start();
  });

  watch(
    () => props.intensity,
    (value) => field?.setIntensity(value ?? 1),
  );

  watch(
    () => props.enabled,
    (enabled) => {
      if (enabled && !document.hidden) field?.start();
      else field?.stop();
    },
  );

  onBeforeUnmount(() => {
    observer?.disconnect();
    observer = null;
    window.removeEventListener("resize", resize);
    document.removeEventListener("visibilitychange", onVisibilityChange);
    field?.destroy();
    field = null;
  });
}
