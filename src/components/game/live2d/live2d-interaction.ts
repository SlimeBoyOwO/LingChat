export interface StageRect {
  left: number
  top: number
  width: number
  height: number
}

export interface StageSize {
  width: number
  height: number
}

export interface StagePoint {
  x: number
  y: number
}

export function pointerToStagePoint(
  clientX: number,
  clientY: number,
  rect: StageRect,
  stage: StageSize,
): StagePoint | null {
  if (rect.width <= 0 || rect.height <= 0 || stage.width <= 0 || stage.height <= 0) return null
  return {
    x: ((clientX - rect.left) / rect.width) * stage.width,
    y: ((clientY - rect.top) / rect.height) * stage.height,
  }
}

export function focusDirection(pointer: StagePoint, anchor: StagePoint): StagePoint {
  const deltaX = pointer.x - anchor.x
  const deltaY = pointer.y - anchor.y
  const length = Math.hypot(deltaX, deltaY)
  if (length < 0.001) return { x: 0, y: 0 }
  return { x: deltaX / length, y: -deltaY / length }
}

export function areEyesOpen(values: number[], threshold = 0.15): boolean {
  return values.length === 0 || values.some((value) => value > threshold)
}
