import { describe, expect, it } from 'vitest'

import { areEyesOpen, focusDirection, pointerToStagePoint } from './live2d-interaction'

describe('pointerToStagePoint', () => {
  it('maps browser coordinates into Pixi world coordinates', () => {
    expect(
      pointerToStagePoint(
        300,
        250,
        { left: 100, top: 50, width: 400, height: 400 },
        { width: 800, height: 600 },
      ),
    ).toEqual({ x: 400, y: 300 })
  })

  it('keeps coordinates outside the stage for directional focus', () => {
    expect(
      pointerToStagePoint(
        0,
        700,
        { left: 100, top: 100, width: 400, height: 400 },
        { width: 800, height: 600 },
      ),
    ).toEqual({ x: -200, y: 900 })
  })

  it('rejects a stage without measurable dimensions', () => {
    expect(
      pointerToStagePoint(
        100,
        100,
        { left: 0, top: 0, width: 0, height: 100 },
        { width: 800, height: 600 },
      ),
    ).toBeNull()
  })
})

describe('focusDirection', () => {
  it('uses the configured eye anchor as the direction origin', () => {
    expect(focusDirection({ x: 500, y: 200 }, { x: 300, y: 200 })).toEqual({ x: 1, y: -0 })
    expect(focusDirection({ x: 300, y: 0 }, { x: 300, y: 200 })).toEqual({ x: 0, y: 1 })
  })

  it('looks forward when the pointer is on the eye anchor', () => {
    expect(focusDirection({ x: 300, y: 200 }, { x: 300, y: 200 })).toEqual({ x: 0, y: 0 })
  })
})

describe('areEyesOpen', () => {
  it('tracks focus while either eye is visibly open', () => {
    expect(areEyesOpen([0, 1])).toBe(true)
    expect(areEyesOpen([0.16, 0.1])).toBe(true)
  })

  it('suspends focus when all configured eyes are closed', () => {
    expect(areEyesOpen([0, 0])).toBe(false)
    expect(areEyesOpen([0.1, 0.15])).toBe(false)
  })

  it('keeps focus available for models without eye-open parameters', () => {
    expect(areEyesOpen([])).toBe(true)
  })
})
