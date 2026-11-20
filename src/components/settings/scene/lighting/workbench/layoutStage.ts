/**
 * 舞台取景计算（纯函数，无 DOM / 无 WebGL）。
 *
 * 从沙箱 `calcBgRect` 搬来：背景不是死板的整幅铺满，而是跟着镜头做**轻微**推拉
 * （系数 0.25，只有人物的零头）——这就是「切景别时场景也要动」的来源。
 *
 * 人物的取景公式保持仓库原样（`charRect`），没有换成沙箱那套垂直锚点：
 * 现有预设是在仓库这套构图下验收过的，换公式等于所有画面重新构图，属于另一件事。
 */

export type Rect4 = [number, number, number, number];

export interface Size {
  width: number;
  height: number;
}

/** 背景跟着镜头推拉的系数：人物是 1:1，背景只吃 25%，形成视差而不穿帮 */
const BG_ZOOM_FACTOR = 0.25;

/** 背景垂直位移系数：构图高低（focus_y）对背景的影响远小于对人物的影响 */
const BG_FOCUS_FACTOR = 0.12;

/**
 * 计算背景在舞台里的目标矩形。
 *
 * @param bg 底图原始尺寸（未知时传舞台尺寸，等价于铺满不缩放）
 * @param stageW 舞台宽（像素）
 * @param stageH 舞台高（像素）
 * @param camZoom 镜头推拉（1 = 基准）
 * @param camFocusY 构图高低（-0.5 ~ 0.8）
 */
export function calcBgRect(
  bg: Size,
  stageW: number,
  stageH: number,
  camZoom: number,
  camFocusY: number,
): Rect4 {
  const bgW = Math.max(1, bg.width);
  const bgH = Math.max(1, bg.height);
  const zoom = Number.isFinite(camZoom) && camZoom > 0 ? camZoom : 1;
  const focus = Number.isFinite(camFocusY) ? camFocusY : 0;

  const bgZoom = 1 + (zoom - 1) * BG_ZOOM_FACTOR;
  // max(...) 保证任何比例下都铺满舞台，不出现露底黑边
  const scale = Math.max(stageW / bgW, stageH / bgH) * bgZoom;
  const w = bgW * scale;
  const h = bgH * scale;
  return [(stageW - w) * 0.5, (stageH - h) * 0.5 + focus * stageH * BG_FOCUS_FACTOR, w, h];
}

/**
 * 计算立绘在舞台里的目标矩形（仓库既有口径，勿与沙箱混用）。
 */
export function calcCharRect(
  charAspect: number,
  stageW: number,
  stageH: number,
  camZoom: number,
  camFocusY: number,
): Rect4 {
  const zoom = Number.isFinite(camZoom) && camZoom > 0 ? camZoom : 1.25;
  const focus = Number.isFinite(camFocusY) ? camFocusY : 0.16;
  const aspect = Number.isFinite(charAspect) && charAspect > 0 ? charAspect : 0.75;

  const h = stageH * 0.92 * zoom;
  const w = h * aspect;
  const x = (stageW - w) * 0.5;
  const y = (stageH - h) * 0.5 + focus * stageH;
  return [x, y, w, h];
}
