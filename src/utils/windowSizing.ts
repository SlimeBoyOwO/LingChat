import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'

const RESIZE_QUIET_MS = 80
const RESIZE_MIN_WAIT_MS = 160
const RESIZE_POLL_MS = 24
const RESIZE_TIMEOUT_MS = 900
const PET_SCALE_MIN = 0.7
const PET_SCALE_MAX = 1.3
const PET_SCALE_DEFAULT = 1
const PET_BASE_WIDTH = 240
const PET_AVATAR_HEIGHT = 240
const PET_DIALOG_HEIGHT = 75
const PET_CHAT_HEIGHT = 45
const PET_SIZE_TOLERANCE_PHYSICAL_PX = 3

let latestRequestId = 0
let operationQueue: Promise<void> = Promise.resolve()
let preparedMode: boolean | null = null
let preparedScale: number | null = null

const delay = (milliseconds: number) =>
  new Promise<void>((resolve) => window.setTimeout(resolve, milliseconds))

export const normalizePetScale = (scale: unknown): number => {
  const numericScale = Number(scale)
  if (!Number.isFinite(numericScale)) return PET_SCALE_DEFAULT
  return Math.min(PET_SCALE_MAX, Math.max(PET_SCALE_MIN, numericScale))
}

const normalizeScale = (scale?: number) =>
  typeof scale === 'number' ? normalizePetScale(scale) : undefined

const expectedPetLogicalSize = (scale: number) => ({
  width: Math.round(PET_BASE_WIDTH * scale),
  height:
    Math.round(PET_AVATAR_HEIGHT * scale) +
    Math.round(PET_DIALOG_HEIGHT * scale) +
    Math.round(PET_CHAT_HEIGHT * scale),
})

export function invalidatePetWindowModePreparation(): void {
  preparedMode = null
  preparedScale = null
}

export function isPetWindowModePrepared(scale?: number): boolean {
  const expectedScale = normalizeScale(scale) ?? PET_SCALE_DEFAULT
  return (
    preparedMode === true &&
    preparedScale !== null &&
    Math.abs(preparedScale - expectedScale) < 0.0001
  )
}

/**
 * Applies a pet-window mode change and waits until Tauri has stopped reporting
 * size changes. Requests are serialized and stale queued requests are skipped,
 * so the last requested scale always wins during rapid slider input.
 */
export function setPetWindowModeAndWait(enable: boolean, scale?: number): Promise<boolean> {
  const requestId = ++latestRequestId

  const operation = operationQueue.then(async () => {
    if (requestId !== latestRequestId) return false

    const appWindow = getCurrentWindow()
    let lastSize = await appWindow.innerSize().catch(() => null)
    let lastChangeAt = Date.now()

    const unlisten = await appWindow.onResized(({ payload }) => {
      lastSize = payload
      lastChangeAt = Date.now()
    })

    try {
      if (requestId !== latestRequestId) return false

      const normalizedScale = normalizeScale(scale)

      // A request in flight must never be reused as an already-prepared route transition.
      invalidatePetWindowModePreparation()

      lastChangeAt = Date.now()
      await invoke('set_pet_mode', {
        enable,
        ...(normalizedScale !== undefined ? { scale: normalizedScale } : {}),
      })

      const startedAt = Date.now()
      let resizeSettled = false
      while (requestId === latestRequestId && Date.now() - startedAt < RESIZE_TIMEOUT_MS) {
        await delay(RESIZE_POLL_MS)

        const currentSize = await appWindow.innerSize().catch(() => lastSize)
        if (
          currentSize &&
          (!lastSize ||
            currentSize.width !== lastSize.width ||
            currentSize.height !== lastSize.height)
        ) {
          lastSize = currentSize
          lastChangeAt = Date.now()
        }

        const elapsed = Date.now() - startedAt
        if (elapsed >= RESIZE_MIN_WAIT_MS && Date.now() - lastChangeAt >= RESIZE_QUIET_MS) {
          resizeSettled = true
          break
        }
      }

      const isLatestRequest = requestId === latestRequestId
      if (!isLatestRequest) return false

      if (!resizeSettled) {
        invalidatePetWindowModePreparation()
        throw new Error(`等待${enable ? '桌宠' : '主'}窗口尺寸稳定超时`)
      }

      if (enable) {
        const finalScale = normalizedScale ?? PET_SCALE_DEFAULT
        const targetLogicalSize = expectedPetLogicalSize(finalScale)
        const [scaleFactor, finalPhysicalSize] = await Promise.all([
          appWindow.scaleFactor(),
          appWindow.innerSize(),
        ])

        if (!Number.isFinite(scaleFactor) || scaleFactor <= 0) {
          invalidatePetWindowModePreparation()
          throw new Error(`无法验证桌宠窗口尺寸：无效的 DPI 缩放系数 ${scaleFactor}`)
        }

        const expectedPhysicalWidth = targetLogicalSize.width * scaleFactor
        const expectedPhysicalHeight = targetLogicalSize.height * scaleFactor
        const widthDeviation = Math.abs(finalPhysicalSize.width - expectedPhysicalWidth)
        const heightDeviation = Math.abs(finalPhysicalSize.height - expectedPhysicalHeight)

        if (
          widthDeviation > PET_SIZE_TOLERANCE_PHYSICAL_PX ||
          heightDeviation > PET_SIZE_TOLERANCE_PHYSICAL_PX
        ) {
          invalidatePetWindowModePreparation()
          throw new Error(
            `桌宠窗口尺寸校验失败：期望约 ${Math.round(expectedPhysicalWidth)}x${Math.round(expectedPhysicalHeight)} 物理像素，实际 ${finalPhysicalSize.width}x${finalPhysicalSize.height}`,
          )
        }

        preparedMode = true
        preparedScale = finalScale
      } else {
        preparedMode = false
        preparedScale = null
      }

      return true
    } catch (error) {
      if (requestId === latestRequestId) {
        invalidatePetWindowModePreparation()
      }
      throw error
    } finally {
      unlisten()
    }
  })

  operationQueue = operation.then(
    () => undefined,
    () => undefined,
  )

  return operation
}
