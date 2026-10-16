import { onBeforeUnmount, onMounted, watch, type Ref } from "vue";

/**
 * 渲染场对外的生命周期。各引擎内部结构完全不同（雨是平铺数组、雾是纹理带、
 * 雪是精灵图集），但挂到画布上要做的事一样，所以抽成这一个接口。
 */
export interface ParticleField {
  resize(): void;
  start(): void;
  stop(): void;
  /** 密度倍率，0 到 2 */
  setIntensity(value: number): void;
  destroy(): void;
}

/** 各特效组件共用的 props */
export interface ParticleProps {
  enabled?: boolean;
  intensity?: number;
}

/**
 * 把渲染场挂到画布上：尺寸观察、生命周期与可见性。
 *
 * 画布尺寸取自父元素，所以主界面铺满整屏、塞进桌宠的小圆头像里也成立。
 * 雨、雷阵雨、雾、雪、樱花都走这一份，它们之间只差一个 field 实例。
 */
export function useParticleField(
  canvasRef: Ref<HTMLCanvasElement | null>,
  props: ParticleProps,
  createField: (canvas: HTMLCanvasElement, intensity: number) => ParticleField,
) {
  let field: ParticleField | null = null;
  let observer: ResizeObserver | null = null;

  const resize = () => field?.resize();

  const onVisibilityChange = () => {
    if (document.hidden) field?.stop();
    else if (props.enabled ?? true) field?.start();
  };

  onMounted(() => {
    const canvas = canvasRef.value;
    if (!canvas) return;

    field = createField(canvas, props.intensity ?? 1);

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
