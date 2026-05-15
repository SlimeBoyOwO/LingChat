import type { Drop } from '../config/rain'

export function useRain() {
  function createDrop(W: number, H: number, intensity: number = 1): Drop {
    return {
      x: Math.random() * W,
      y: Math.random() * H - 100,
      speed: 6 + Math.random() * 8 * intensity,
      length: Math.random() * 5 * intensity + 20,
    }
  }

  return {
    createDrop,
  }
}
