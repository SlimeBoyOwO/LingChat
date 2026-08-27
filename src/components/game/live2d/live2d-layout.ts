export interface Live2dLayoutBounds {
  width: number
  height: number
}

export interface Live2dLayoutScreen {
  width: number
  height: number
}

export interface Live2dPetLayout {
  anchorX: number
  anchorY: number
  scale: number
  x: number
  y: number
}

export function calculatePetLayout(
  screen: Live2dLayoutScreen,
  bounds: Live2dLayoutBounds,
  roleScale = 1,
  offsetX = 0,
  offsetY = 0,
): Live2dPetLayout {
  const width = bounds.width || 1
  const height = bounds.height || 1
  const coverScale = Math.max(screen.width / width, screen.height / height)
  return {
    anchorX: 0.5,
    anchorY: 0,
    scale: coverScale * roleScale,
    x: screen.width / 2 + offsetX,
    y: offsetY,
  }
}
