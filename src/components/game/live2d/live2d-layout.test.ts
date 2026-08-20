import { describe, expect, it } from 'vitest'

import { calculatePetLayout } from './live2d-layout'

describe('calculatePetLayout', () => {
  it('covers the pet frame while keeping a portrait model top-aligned', () => {
    expect(calculatePetLayout({ width: 210, height: 210 }, { width: 472, height: 1082 })).toEqual({
      anchorX: 0.5,
      anchorY: 0,
      scale: 210 / 472,
      x: 105,
      y: 0,
    })
  })

  it('uses height as the cover constraint for a landscape model', () => {
    expect(calculatePetLayout({ width: 200, height: 200 }, { width: 800, height: 400 })).toEqual({
      anchorX: 0.5,
      anchorY: 0,
      scale: 0.5,
      x: 100,
      y: 0,
    })
  })

  it('applies role scale and offsets without changing the top anchor', () => {
    expect(
      calculatePetLayout({ width: 240, height: 240 }, { width: 600, height: 900 }, 1.25, 8, -12),
    ).toEqual({
      anchorX: 0.5,
      anchorY: 0,
      scale: 0.5,
      x: 128,
      y: -12,
    })
  })
})
