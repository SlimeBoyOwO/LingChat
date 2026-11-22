/**
 * 工作台的视图态。
 *
 * 与 useSceneLighting 的文档态（可导出、可撤销）严格分开：
 * 这里的东西刷新页面就没了，也不该进 JSON。
 */
import { ref } from "vue";

export type RightTab = "props" | "history";
export type CanvasTool = "select" | "hand" | "light" | "marquee";

export interface ParamTarget {
  group: "light" | "global" | "compositing" | "material" | "camera";
  key: string;
}

export function useWorkbenchUi() {
  const rightTab = ref<RightTab>("props");
  const canvasTool = ref<CanvasTool>("select");
  /** 画布上是否显示光源指示点 */
  const showGizmo = ref(true);
  const showGrid = ref(false);
  /** 画布缩放：1 = 适应画布（fit），>1 放大 */
  const zoom = ref(1);
  /** 右栏两组折叠区（「更多参数」「高级参数」） */
  const expandAir = ref(false);
  const expandMaterial = ref(false);

  // —— 轻提示 ——
  const toastMessage = ref("");
  let toastTimer: ReturnType<typeof setTimeout> | null = null;
  function toast(msg: string) {
    toastMessage.value = msg;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toastMessage.value = "";
    }, 2000);
  }

  return {
    rightTab,
    canvasTool,
    showGizmo,
    showGrid,
    zoom,
    expandAir,
    expandMaterial,
    toastMessage,
    toast,
  };
}
