/** 平面点。本文件的运算统一在「调用方给的同一个坐标系」里做，
    桌宠模式下即窗口视口坐标（y 向下）。 */
export interface StagePoint {
  x: number;
  y: number;
}

/** 矩形区域，与指针同一坐标系。桌宠模式下是当前显示器的工作区
    （排除任务栏，与 restore_normal_geometry 取 work_area 的约定一致）。 */
export interface ScreenBox {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface GazeVector {
  /** 单位方向，已按 Live2D 约定取反 y（屏幕向下 → 模型 -Y） */
  x: number;
  y: number;
  /** 0..1：锚点到指针的距离 ÷ 该方向上锚点到屏幕边缘的距离 */
  magnitude: number;
}

/**
 * 参考距离下限占工作区短边的比例。
 *
 * 桌宠常被停在屏幕角落，此时朝角落方向到屏幕边缘可能只剩几十像素，
 * 不设下限的话那个方向一有位移就满偏——正是这次要修的「头部永远满偏」。
 */
export const GAZE_REFERENCE_RATIO = 0.35;

/**
 * 焦点幅度下限（配合 Live2DStage 的瞳孔补偿使用）。
 *
 * 幅度趋近 0 时补偿量 1/m 会发散，焦点弹簧的逐轴死区（EPSILON = 0.01）
 * 带来的瞳孔误差也随之放大。0.08 把误差封顶在约 18%、把残余头部偏航
 * 封顶在约 2.4°（不可察觉）。
 */
export const GAZE_MAGNITUDE_MIN = 0.08;

/** 单轴 slab 区间；与该轴平行且原点在区间外时返回 null（射线永远出不去）。 */
function slabRange(origin: number, direction: number, min: number, max: number) {
  if (Math.abs(direction) < 1e-6) {
    return origin < min || origin > max ? null : ([-Infinity, Infinity] as const);
  }
  const first = (min - origin) / direction;
  const second = (max - origin) / direction;
  return [Math.min(first, second), Math.max(first, second)] as const;
}

/** 沿 unit 方向从 origin 出发到矩形边界的距离；不相交返回 null。
    origin 与 unit 都必须是 y 向下的屏幕坐标系。 */
function distanceToBoxEdge(origin: StagePoint, unit: StagePoint, box: ScreenBox): number | null {
  const horizontal = slabRange(origin.x, unit.x, box.left, box.left + box.width);
  if (!horizontal) return null;
  const vertical = slabRange(origin.y, unit.y, box.top, box.top + box.height);
  if (!vertical) return null;
  const near = Math.max(horizontal[0], vertical[0]);
  const far = Math.min(horizontal[1], vertical[1]);
  if (near > far) return null;
  // 原点在矩形内（near < 0 < far）取出口 far；原点在矩形外（窗口被拖出屏幕）取进入点 near
  if (near > 0) return near;
  return far > 0 ? far : null;
}

/** 拿不到工作区矩形时的径向参考距离。
    尺寸在多显示器混合 DPI 下是可靠的，位置不是，所以只取尺寸。 */
export function radialReferenceDistance(width: number, height: number): number {
  return GAZE_REFERENCE_RATIO * Math.min(width, height);
}

/**
 * 指针 + 锚点 → 视线向量。
 *
 * 方向恒为单位向量，交给引擎做瞳孔的满幅追踪；magnitude 用来按比例衰减
 * 头部旋转，使鼠标贴近桌宠时头基本回正、移到屏幕边缘时才转到满偏。
 *
 * 参考距离取「锚点沿该方向到工作区边界的距离」，并带下限
 * （GAZE_REFERENCE_RATIO × 工作区短边，见该常量）。射线打不到工作区
 * （锚点被拖出屏幕等）时回落到下限，**绝不回落到满偏**——那等于把
 * 「头部永远满偏」原样放回来。
 *
 * 全部运算在 y 向下的屏幕坐标系里完成，只在这里取反一次 y。slab 运算
 * 必须用取反前的方向，否则参考距离会沿垂直方向被镜像。
 */
export function gazeFromPointer(
  pointer: StagePoint,
  anchor: StagePoint,
  box: ScreenBox | null,
  fallbackReference: number,
): GazeVector {
  const deltaX = pointer.x - anchor.x;
  const deltaY = pointer.y - anchor.y;
  const distance = Math.hypot(deltaX, deltaY);
  if (distance < 0.5) return { x: 0, y: 0, magnitude: 0 };
  const unitX = deltaX / distance;
  const unitY = deltaY / distance;
  // 「+ 0」把 -0 归一成 0：轴对齐方向会算出 -0，虽然算术上无害，
  // 但会让 Object.is 系的断言（以及任何序列化）莫名其妙地失败
  const directionX = unitX + 0;
  const directionY = -unitY + 0;
  const floor = box ? GAZE_REFERENCE_RATIO * Math.min(box.width, box.height) : fallbackReference;
  const edge = box ? distanceToBoxEdge(anchor, { x: unitX, y: unitY }, box) : null;
  const reference = Math.max(edge ?? 0, floor);
  // 完全拿不到屏幕信息（浏览器 dev / 移动端且 window.screen 也不可用）：维持旧行为
  if (reference < 1) return { x: directionX, y: directionY, magnitude: 1 };
  return { x: directionX, y: directionY, magnitude: Math.min(1, distance / reference) };
}

export function areEyesOpen(values: number[], threshold = 0.15): boolean {
  return values.length === 0 || values.some((value) => value > threshold);
}
